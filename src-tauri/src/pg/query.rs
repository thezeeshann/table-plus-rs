//! Query execution using the simple-query protocol so every value arrives as
//! the server's text rendering (exactly what TablePlus shows in the grid).

use crate::error::AppResult;
use crate::model::{QueryResult, ResultColumn};
use crate::pg::sqlgen::{leading_keyword, split_statements};
use futures_util::{pin_mut, TryStreamExt};
use std::time::Instant;
use tokio_postgres::{Client, SimpleQueryMessage};

pub const MAX_RESULT_ROWS: usize = 100_000;

/// Column types for a statement via a throwaway prepare. Best-effort: utility
/// statements that can't be prepared just yield name-only columns later.
async fn describe(client: &Client, sql: &str) -> Option<Vec<ResultColumn>> {
    let stmt = client.prepare(sql).await.ok()?;
    Some(
        stmt.columns()
            .iter()
            .map(|c| ResultColumn {
                name: c.name().to_string(),
                type_name: c.type_().name().to_string(),
                type_oid: c.type_().oid(),
            })
            .collect(),
    )
}

/// Executes one statement and collects every result set it produces.
pub async fn run_statement(client: &Client, sql: &str, row_limit: usize) -> AppResult<Vec<QueryResult>> {
    let kw = leading_keyword(sql);
    let wants_types = matches!(
        kw.as_str(),
        "SELECT" | "WITH" | "TABLE" | "VALUES" | "SHOW" | "EXPLAIN" | "INSERT" | "UPDATE" | "DELETE" | "MERGE"
    );
    let typed = if wants_types { describe(client, sql).await } else { None };

    let started = Instant::now();
    let stream = client.simple_query_raw(sql).await;
    let mut results: Vec<QueryResult> = Vec::new();
    let mut current: Option<QueryResult> = None;

    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            let err: crate::error::AppError = e.into();
            return Ok(vec![QueryResult {
                statement: sql.to_string(),
                columns: vec![],
                rows: vec![],
                rows_affected: None,
                command: Some(kw),
                duration_ms: started.elapsed().as_secs_f64() * 1000.0,
                error: Some(serde_json::to_value(&err)?),
                truncated: false,
            }]);
        }
    };
    pin_mut!(stream);
    let mut got_error: Option<serde_json::Value> = None;
    loop {
        match stream.try_next().await {
            Ok(Some(msg)) => match msg {
                SimpleQueryMessage::RowDescription(cols) => {
                    if let Some(c) = current.take() {
                        results.push(c);
                    }
                    let columns: Vec<ResultColumn> = match &typed {
                        Some(t) if t.len() == cols.len() => t.clone(),
                        _ => cols
                            .iter()
                            .map(|c| ResultColumn { name: c.name().to_string(), type_name: "text".into(), type_oid: 0 })
                            .collect(),
                    };
                    current = Some(QueryResult {
                        statement: sql.to_string(),
                        columns,
                        rows: Vec::new(),
                        rows_affected: None,
                        command: Some(kw.clone()),
                        duration_ms: 0.0,
                        error: None,
                        truncated: false,
                    });
                }
                SimpleQueryMessage::Row(row) => {
                    if let Some(cur) = current.as_mut() {
                        if cur.rows.len() >= row_limit {
                            cur.truncated = true;
                            continue;
                        }
                        let mut vals = Vec::with_capacity(row.len());
                        for i in 0..row.len() {
                            vals.push(row.get(i).map(|s| s.to_string()));
                        }
                        cur.rows.push(vals);
                    }
                }
                SimpleQueryMessage::CommandComplete(n) => {
                    let mut r = current.take().unwrap_or_else(|| QueryResult {
                        statement: sql.to_string(),
                        columns: vec![],
                        rows: vec![],
                        rows_affected: None,
                        command: Some(kw.clone()),
                        duration_ms: 0.0,
                        error: None,
                        truncated: false,
                    });
                    r.rows_affected = Some(n);
                    r.duration_ms = started.elapsed().as_secs_f64() * 1000.0;
                    results.push(r);
                }
                _ => {}
            },
            Ok(None) => break,
            Err(e) => {
                let err: crate::error::AppError = e.into();
                got_error = Some(serde_json::to_value(&err)?);
                break;
            }
        }
    }
    if let Some(c) = current.take() {
        results.push(c);
    }
    if let Some(err) = got_error {
        results.push(QueryResult {
            statement: sql.to_string(),
            columns: vec![],
            rows: vec![],
            rows_affected: None,
            command: Some(kw),
            duration_ms: started.elapsed().as_secs_f64() * 1000.0,
            error: Some(err),
            truncated: false,
        });
    }
    for r in results.iter_mut() {
        if r.duration_ms == 0.0 {
            r.duration_ms = started.elapsed().as_secs_f64() * 1000.0;
        }
    }
    Ok(results)
}

/// Runs a whole script statement by statement, stopping at the first error
/// (TablePlus behaviour for "Run All").
pub async fn run_script(client: &Client, script: &str, row_limit: usize, stop_on_error: bool) -> AppResult<Vec<QueryResult>> {
    let mut all = Vec::new();
    for (stmt, _, _) in split_statements(script) {
        let res = run_statement(client, &stmt, row_limit).await?;
        let failed = res.iter().any(|r| r.error.is_some());
        all.extend(res);
        if failed && stop_on_error {
            break;
        }
    }
    Ok(all)
}
