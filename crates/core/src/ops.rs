//! Everything the UI asks a session to do. Each method takes the session's
//! `exec_lock`, so a slow request never interleaves with another on the same
//! connection (required behind transaction poolers).

use crate::error::{AppError, AppResult};
use crate::model::*;
use crate::pg::catalog;
use crate::pg::ddl::{ddl_sql, DdlOp};
use crate::pg::query::{run_script, run_statement, MAX_RESULT_ROWS};
use crate::pg::session::Session;
use crate::pg::sqlgen::{build_order, build_where, generate_change_sql, qualified};
use std::collections::BTreeMap;
use std::time::Instant;
use tokio_postgres::types::Type;

fn supports_ctid(kind: &str) -> bool {
    matches!(kind, "table" | "partitioned")
}

/// Hidden column that carries each row's physical id to the grid.
pub const CTID_COLUMN: &str = "__table_plus_rs_ctid";

impl Session {
    pub fn record_history(&self, sql: &str, duration_ms: f64, ok: bool, message: Option<String>, rows: Option<u64>) {
        let Some(storage) = &self.storage else { return };
        let entry = HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            connection_id: self.info.connection_id.clone(),
            database: Some(self.info.database.clone()),
            sql: sql.to_string(),
            at: chrono::Local::now().to_rfc3339(),
            duration_ms,
            ok,
            message,
            rows,
        };
        if let Err(e) = storage.append_history(&entry) {
            log::warn!("history write failed: {e}");
        }
    }

    pub async fn list_databases(&self) -> AppResult<Vec<String>> {
        let _g = self.exec_lock.lock().await;
        catalog::list_databases(&self.client).await
    }

    pub async fn list_schemas(&self) -> AppResult<Vec<String>> {
        let _g = self.exec_lock.lock().await;
        catalog::list_schemas(&self.client, self.config.show_system_schemas).await
    }

    pub async fn list_items(&self, schema: &str) -> AppResult<SchemaListing> {
        let _g = self.exec_lock.lock().await;
        catalog::list_items(&self.client, schema, self.info.server_version_num).await
    }

    pub async fn table_structure(&self, schema: &str, table: &str, kind: &str) -> AppResult<TableStructure> {
        let _g = self.exec_lock.lock().await;
        catalog::table_structure(&self.client, schema, table, kind, self.info.server_version_num).await
    }

    pub async fn object_ddl(&self, schema: &str, name: &str, kind: &str) -> AppResult<String> {
        let _g = self.exec_lock.lock().await;
        catalog::object_ddl(&self.client, schema, name, kind, self.info.server_version_num).await
    }

    pub async fn processes(&self) -> AppResult<Vec<ProcessInfo>> {
        let _g = self.exec_lock.lock().await;
        catalog::processes(&self.client).await
    }

    /// Deliberately skips `exec_lock`: cancelling must work while a query holds it.
    pub async fn kill_process(&self, pid: i32, terminate: bool) -> AppResult<bool> {
        let sql = if terminate { "SELECT pg_terminate_backend($1)" } else { "SELECT pg_cancel_backend($1)" };
        let row = self.client.query_typed_one(sql, &[(&pid, Type::INT4)]).await?;
        Ok(row.get(0))
    }

    /// Column names for every relation in a schema; feeds editor autocompletion.
    pub async fn schema_columns(&self, schema: &str) -> AppResult<BTreeMap<String, Vec<String>>> {
        let _g = self.exec_lock.lock().await;
        let rows = self
            .client
            .query_typed(
                "SELECT c.relname::text, a.attname::text FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1 AND a.attnum > 0 AND NOT a.attisdropped \
                 AND c.relkind IN ('r','v','m','f','p') ORDER BY c.relname, a.attnum",
                &[(&schema, Type::TEXT)],
            )
            .await?;
        let mut map: BTreeMap<String, Vec<String>> = Default::default();
        for r in rows {
            map.entry(r.get(0)).or_default().push(r.get(1));
        }
        Ok(map)
    }

    pub async fn fetch_rows(&self, req: &FetchRowsRequest, estimate_threshold: i64) -> AppResult<FetchRowsResponse> {
        let _guard = self.exec_lock.lock().await;
        let rel = qualified(&req.schema, &req.table);

        let text_cols: Vec<String> = if req.quick_filter.as_deref().map(|q| !q.trim().is_empty()).unwrap_or(false) {
            self.client
                .query_typed(
                    "SELECT a.attname::text FROM pg_attribute a WHERE a.attrelid = (quote_ident($1::text) || '.' || quote_ident($2::text))::regclass AND a.attnum > 0 AND NOT a.attisdropped",
                    &[(&req.schema, Type::TEXT), (&req.table, Type::TEXT)],
                )
                .await?
                .iter()
                .map(|r| r.get(0))
                .collect()
        } else {
            Vec::new()
        };
        let where_ = build_where(&req.filters, req.quick_filter.as_deref(), &text_cols)?;
        let order = build_order(&req.sort);
        let ctid = supports_ctid(&req.kind);

        let mut sql = format!("SELECT {}* FROM {rel}", if ctid { format!("ctid::text AS \"{CTID_COLUMN}\", ") } else { String::new() });
        if !where_.is_empty() {
            sql.push_str(&format!(" WHERE {where_}"));
        }
        if !order.is_empty() {
            sql.push_str(&format!(" ORDER BY {order}"));
        }
        sql.push_str(&format!(" LIMIT {} OFFSET {}", req.limit.max(1), req.offset));

        let started = Instant::now();
        let mut results = run_statement(&self.client, &sql, req.limit as usize + 1, self.supports_prepare).await?;
        let res = results.pop().ok_or_else(|| AppError::msg("no result"))?;
        if let Some(err) = res.error {
            return Err(AppError::Message(err.message));
        }

        let (columns, rows, ctids) = if ctid {
            let cols = res.columns.into_iter().skip(1).collect::<Vec<_>>();
            let mut ctids = Vec::with_capacity(res.rows.len());
            let rows = res
                .rows
                .into_iter()
                .map(|mut r| {
                    ctids.push(r.remove(0).unwrap_or_default());
                    r
                })
                .collect();
            (cols, rows, Some(ctids))
        } else {
            (res.columns, res.rows, None)
        };

        // Row count: exact when filtered or small; estimate (reltuples) for large tables,
        // matching TablePlus' "Estimate count when table has more than N rows" setting.
        let mode = if req.count_mode.is_empty() { "auto" } else { req.count_mode.as_str() };
        let (total, is_estimate) = match mode {
            "none" => (None, false),
            _ => {
                let est = if ctid { catalog::estimated_rows(&self.client, &req.schema, &req.table).await.unwrap_or(None) } else { None };
                let want_exact = mode == "exact" || !where_.is_empty() || est.map(|e| e < estimate_threshold).unwrap_or(true);
                if want_exact && mode != "estimate" {
                    let csql = format!(
                        "SELECT count(*) FROM {rel}{}",
                        if where_.is_empty() { String::new() } else { format!(" WHERE {where_}") }
                    );
                    match self.client.query_typed_one(&csql, &[]).await {
                        Ok(r) => (Some(r.get::<_, i64>(0)), false),
                        Err(_) => (est, true),
                    }
                } else {
                    (est, true)
                }
            }
        };

        Ok(FetchRowsResponse {
            columns,
            rows,
            ctids,
            total,
            total_is_estimate: is_estimate,
            duration_ms: started.elapsed().as_secs_f64() * 1000.0,
            sql,
        })
    }

    /// Applies grid edits inside one transaction; any failure rolls everything back.
    pub async fn apply_changes(&self, req: &ApplyChangesRequest) -> AppResult<Vec<String>> {
        let statements = generate_change_sql(&req.schema, &req.table, &req.primary_key, &req.changes)?;
        self.run_in_transaction(statements).await
    }

    pub async fn apply_ddl(&self, schema: &str, table: &str, ops: &[DdlOp]) -> AppResult<Vec<String>> {
        self.run_in_transaction(ddl_sql(schema, table, ops)).await
    }

    pub async fn run_in_transaction(&self, statements: Vec<String>) -> AppResult<Vec<String>> {
        let _guard = self.exec_lock.lock().await;
        if statements.is_empty() {
            return Ok(vec![]);
        }
        self.client.batch_execute("BEGIN").await?;
        for stmt in &statements {
            if let Err(e) = self.client.batch_execute(stmt).await {
                let _ = self.client.batch_execute("ROLLBACK").await;
                self.record_history(&statements.join(";\n"), 0.0, false, Some(e.to_string()), None);
                return Err(e.into());
            }
        }
        self.client.batch_execute("COMMIT").await?;
        self.record_history(&statements.join(";\n"), 0.0, true, None, Some(statements.len() as u64));
        Ok(statements)
    }

    /// `split` = Run All (statement by statement); otherwise the text runs as one statement.
    pub async fn run_query(&self, sql: &str, split: bool, row_limit: Option<usize>, record: bool) -> AppResult<Vec<QueryResult>> {
        let _guard = self.exec_lock.lock().await;
        let limit = row_limit.unwrap_or(MAX_RESULT_ROWS).min(MAX_RESULT_ROWS);
        let results = if split {
            run_script(&self.client, sql, limit, true, self.supports_prepare).await?
        } else {
            run_statement(&self.client, sql.trim(), limit, self.supports_prepare).await?
        };
        if record {
            for r in &results {
                self.record_history(&r.statement, r.duration_ms, r.error.is_none(), r.error.as_ref().map(|e| e.message.clone()), r.rows_affected);
            }
        }
        Ok(results)
    }

    /// Runs a statement for its side effect (drop/truncate/refresh matview…).
    pub async fn execute(&self, sql: &str) -> AppResult<u64> {
        let _guard = self.exec_lock.lock().await;
        let results = run_statement(&self.client, sql.trim(), 1, false).await?;
        let mut affected = 0u64;
        for r in results {
            if let Some(e) = r.error {
                self.record_history(sql, r.duration_ms, false, Some(e.message.clone()), None);
                return Err(AppError::Message(e.message));
            }
            affected += r.rows_affected.unwrap_or(0);
            self.record_history(sql, r.duration_ms, true, None, r.rows_affected);
        }
        Ok(affected)
    }
}
