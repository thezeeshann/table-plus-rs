use crate::error::{AppError, AppResult};
use crate::model::*;
use crate::pg::catalog;
use crate::pg::query::run_statement;
use crate::pg::sqlgen::{build_order, build_where, generate_change_sql, qualified, quote_ident, quote_literal};
use crate::AppState;
use std::time::Instant;
use tauri::State;

fn supports_ctid(kind: &str) -> bool {
    matches!(kind, "table" | "partitioned")
}

#[tauri::command]
pub async fn fetch_rows(state: State<'_, AppState>, req: FetchRowsRequest) -> AppResult<FetchRowsResponse> {
    let s = state.sessions.get(&req.session_id).await?;
    let _guard = s.exec_lock.lock().await;
    let rel = qualified(&req.schema, &req.table);
    let settings = state.storage.settings().unwrap_or_default();

    // columns used by quick filter: everything castable to text (i.e. everything)
    let text_cols: Vec<String> = if req.quick_filter.as_deref().map(|q| !q.trim().is_empty()).unwrap_or(false) {
        s.client
            .query(
                "SELECT a.attname::text FROM pg_attribute a WHERE a.attrelid = (quote_ident($1::text) || '.' || quote_ident($2::text))::regclass AND a.attnum > 0 AND NOT a.attisdropped",
                &[&req.schema, &req.table],
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

    let mut sql = format!(
        "SELECT {}* FROM {rel}",
        if ctid { "ctid::text AS \"__tusk_ctid\", " } else { "" }
    );
    if !where_.is_empty() {
        sql.push_str(&format!(" WHERE {where_}"));
    }
    if !order.is_empty() {
        sql.push_str(&format!(" ORDER BY {order}"));
    }
    sql.push_str(&format!(" LIMIT {} OFFSET {}", req.limit.max(1), req.offset));

    let started = Instant::now();
    let mut results = run_statement(&s.client, &sql, req.limit as usize + 1).await?;
    let res = results.pop().ok_or_else(|| AppError::msg("no result"))?;
    if let Some(err) = res.error {
        return Err(AppError::Message(
            err.get("message").and_then(|m| m.as_str()).unwrap_or("query failed").to_string(),
        ));
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
            let est = if ctid { catalog::estimated_rows(&s.client, &req.schema, &req.table).await.unwrap_or(None) } else { None };
            let want_exact = mode == "exact"
                || !where_.is_empty()
                || est.map(|e| e < settings.estimate_count_threshold).unwrap_or(true);
            if want_exact && mode != "estimate" {
                let csql = format!(
                    "SELECT count(*) FROM {rel}{}",
                    if where_.is_empty() { String::new() } else { format!(" WHERE {where_}") }
                );
                match s.client.query_one(&csql, &[]).await {
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

#[tauri::command]
pub async fn preview_changes(req: ApplyChangesRequest) -> AppResult<Vec<String>> {
    generate_change_sql(&req.schema, &req.table, &req.primary_key, &req.changes)
}

/// Applies grid edits inside one transaction; any failure rolls everything back.
#[tauri::command]
pub async fn apply_changes(state: State<'_, AppState>, req: ApplyChangesRequest) -> AppResult<Vec<String>> {
    let s = state.sessions.get(&req.session_id).await?;
    let _guard = s.exec_lock.lock().await;
    let statements = generate_change_sql(&req.schema, &req.table, &req.primary_key, &req.changes)?;
    if statements.is_empty() {
        return Ok(vec![]);
    }
    s.client.batch_execute("BEGIN").await?;
    for stmt in &statements {
        if let Err(e) = s.client.batch_execute(stmt).await {
            let _ = s.client.batch_execute("ROLLBACK").await;
            record_history(&state, &s.info, &statements.join(";\n"), 0.0, false, Some(e.to_string()), None);
            return Err(e.into());
        }
    }
    s.client.batch_execute("COMMIT").await?;
    record_history(&state, &s.info, &statements.join(";\n"), 0.0, true, None, Some(statements.len() as u64));
    Ok(statements)
}

pub fn record_history(
    state: &State<'_, AppState>,
    info: &SessionInfo,
    sql: &str,
    duration_ms: f64,
    ok: bool,
    message: Option<String>,
    rows: Option<u64>,
) {
    let entry = HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        connection_id: info.connection_id.clone(),
        database: Some(info.database.clone()),
        sql: sql.to_string(),
        at: chrono::Local::now().to_rfc3339(),
        duration_ms,
        ok,
        message,
        rows,
    };
    if let Err(e) = state.storage.append_history(&entry) {
        log::warn!("history write failed: {e}");
    }
}

/// Structure-tab DDL. Each op maps to one ALTER/COMMENT statement.
#[derive(serde::Deserialize, Debug)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DdlOp {
    AddColumn { name: String, data_type: String, nullable: bool, default: Option<String>, comment: Option<String> },
    DropColumn { name: String },
    RenameColumn { name: String, new_name: String },
    ChangeType { name: String, data_type: String, using: Option<String> },
    SetNullable { name: String, nullable: bool },
    SetDefault { name: String, default: Option<String> },
    SetComment { name: String, comment: Option<String> },
    SetTableComment { comment: Option<String> },
    RenameTable { new_name: String },
    AddIndex { name: String, columns: Vec<String>, unique: bool, method: Option<String>, condition: Option<String> },
    DropIndex { name: String },
    AddPrimaryKey { columns: Vec<String> },
    DropConstraint { name: String },
    AddForeignKey { name: Option<String>, columns: Vec<String>, ref_schema: String, ref_table: String, ref_columns: Vec<String>, on_update: Option<String>, on_delete: Option<String> },
    Truncate { cascade: bool, restart_identity: bool },
    DropTable { cascade: bool },
    CreateTable { name: String, columns: Vec<NewColumn> },
    Raw { sql: String },
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary_key: bool,
}

pub fn ddl_sql(schema: &str, table: &str, ops: &[DdlOp]) -> Vec<String> {
    let rel = qualified(schema, table);
    let mut out = Vec::new();
    for op in ops {
        match op {
            DdlOp::AddColumn { name, data_type, nullable, default, comment } => {
                let mut s = format!("ALTER TABLE {rel} ADD COLUMN {} {data_type}", quote_ident(name));
                if let Some(d) = default.as_deref().filter(|d| !d.trim().is_empty()) {
                    s.push_str(&format!(" DEFAULT {d}"));
                }
                if !nullable {
                    s.push_str(" NOT NULL");
                }
                out.push(s);
                if let Some(c) = comment.as_deref().filter(|c| !c.is_empty()) {
                    out.push(format!("COMMENT ON COLUMN {rel}.{} IS {}", quote_ident(name), quote_literal(c)));
                }
            }
            DdlOp::DropColumn { name } => out.push(format!("ALTER TABLE {rel} DROP COLUMN {}", quote_ident(name))),
            DdlOp::RenameColumn { name, new_name } => {
                out.push(format!("ALTER TABLE {rel} RENAME COLUMN {} TO {}", quote_ident(name), quote_ident(new_name)))
            }
            DdlOp::ChangeType { name, data_type, using } => {
                let mut s = format!("ALTER TABLE {rel} ALTER COLUMN {} TYPE {data_type}", quote_ident(name));
                if let Some(u) = using.as_deref().filter(|u| !u.trim().is_empty()) {
                    s.push_str(&format!(" USING {u}"));
                }
                out.push(s)
            }
            DdlOp::SetNullable { name, nullable } => out.push(format!(
                "ALTER TABLE {rel} ALTER COLUMN {} {} NOT NULL",
                quote_ident(name),
                if *nullable { "DROP" } else { "SET" }
            )),
            DdlOp::SetDefault { name, default } => match default.as_deref().filter(|d| !d.trim().is_empty()) {
                Some(d) => out.push(format!("ALTER TABLE {rel} ALTER COLUMN {} SET DEFAULT {d}", quote_ident(name))),
                None => out.push(format!("ALTER TABLE {rel} ALTER COLUMN {} DROP DEFAULT", quote_ident(name))),
            },
            DdlOp::SetComment { name, comment } => out.push(format!(
                "COMMENT ON COLUMN {rel}.{} IS {}",
                quote_ident(name),
                comment.as_deref().filter(|c| !c.is_empty()).map(quote_literal).unwrap_or_else(|| "NULL".into())
            )),
            DdlOp::SetTableComment { comment } => out.push(format!(
                "COMMENT ON TABLE {rel} IS {}",
                comment.as_deref().filter(|c| !c.is_empty()).map(quote_literal).unwrap_or_else(|| "NULL".into())
            )),
            DdlOp::RenameTable { new_name } => out.push(format!("ALTER TABLE {rel} RENAME TO {}", quote_ident(new_name))),
            DdlOp::AddIndex { name, columns, unique, method, condition } => {
                let mut s = format!(
                    "CREATE {}INDEX {} ON {rel}{} ({})",
                    if *unique { "UNIQUE " } else { "" },
                    quote_ident(name),
                    method.as_deref().filter(|m| !m.is_empty() && !m.eq_ignore_ascii_case("btree")).map(|m| format!(" USING {m}")).unwrap_or_default(),
                    columns.iter().map(|c| if c.contains('(') || c.contains(' ') { c.clone() } else { quote_ident(c) }).collect::<Vec<_>>().join(", ")
                );
                if let Some(w) = condition.as_deref().filter(|c| !c.trim().is_empty()) {
                    s.push_str(&format!(" WHERE {w}"));
                }
                out.push(s)
            }
            DdlOp::DropIndex { name } => out.push(format!("DROP INDEX {}", qualified(schema, name))),
            DdlOp::AddPrimaryKey { columns } => out.push(format!(
                "ALTER TABLE {rel} ADD PRIMARY KEY ({})",
                columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ")
            )),
            DdlOp::DropConstraint { name } => out.push(format!("ALTER TABLE {rel} DROP CONSTRAINT {}", quote_ident(name))),
            DdlOp::AddForeignKey { name, columns, ref_schema, ref_table, ref_columns, on_update, on_delete } => {
                let mut s = format!("ALTER TABLE {rel} ADD ");
                if let Some(n) = name.as_deref().filter(|n| !n.is_empty()) {
                    s.push_str(&format!("CONSTRAINT {} ", quote_ident(n)));
                }
                s.push_str(&format!(
                    "FOREIGN KEY ({}) REFERENCES {} ({})",
                    columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", "),
                    qualified(ref_schema, ref_table),
                    ref_columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ")
                ));
                if let Some(u) = on_update.as_deref().filter(|u| !u.is_empty()) {
                    s.push_str(&format!(" ON UPDATE {u}"));
                }
                if let Some(d) = on_delete.as_deref().filter(|d| !d.is_empty()) {
                    s.push_str(&format!(" ON DELETE {d}"));
                }
                out.push(s)
            }
            DdlOp::Truncate { cascade, restart_identity } => out.push(format!(
                "TRUNCATE TABLE {rel}{}{}",
                if *restart_identity { " RESTART IDENTITY" } else { "" },
                if *cascade { " CASCADE" } else { "" }
            )),
            DdlOp::DropTable { cascade } => out.push(format!("DROP TABLE {rel}{}", if *cascade { " CASCADE" } else { "" })),
            DdlOp::CreateTable { name, columns } => {
                let mut lines: Vec<String> = columns
                    .iter()
                    .map(|c| {
                        let mut l = format!("    {} {}", quote_ident(&c.name), c.data_type);
                        if let Some(d) = c.default.as_deref().filter(|d| !d.trim().is_empty()) {
                            l.push_str(&format!(" DEFAULT {d}"));
                        }
                        if !c.nullable {
                            l.push_str(" NOT NULL");
                        }
                        l
                    })
                    .collect();
                let pk: Vec<String> = columns.iter().filter(|c| c.primary_key).map(|c| quote_ident(&c.name)).collect();
                if !pk.is_empty() {
                    lines.push(format!("    PRIMARY KEY ({})", pk.join(", ")));
                }
                out.push(format!("CREATE TABLE {} (\n{}\n)", qualified(schema, name), lines.join(",\n")));
            }
            DdlOp::Raw { sql } => out.push(sql.clone()),
        }
    }
    out
}

#[tauri::command]
pub async fn preview_ddl(schema: String, table: String, ops: Vec<DdlOp>) -> AppResult<Vec<String>> {
    Ok(ddl_sql(&schema, &table, &ops))
}

#[tauri::command]
pub async fn apply_ddl(state: State<'_, AppState>, session_id: String, schema: String, table: String, ops: Vec<DdlOp>) -> AppResult<Vec<String>> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    let statements = ddl_sql(&schema, &table, &ops);
    if statements.is_empty() {
        return Ok(vec![]);
    }
    s.client.batch_execute("BEGIN").await?;
    for stmt in &statements {
        if let Err(e) = s.client.batch_execute(stmt).await {
            let _ = s.client.batch_execute("ROLLBACK").await;
            record_history(&state, &s.info, stmt, 0.0, false, Some(e.to_string()), None);
            return Err(e.into());
        }
    }
    s.client.batch_execute("COMMIT").await?;
    record_history(&state, &s.info, &statements.join(";\n"), 0.0, true, None, None);
    Ok(statements)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub path: String,
    pub format: String, // csv | json | sql | markdown
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default = "default_true")]
    pub header: bool,
}
fn default_true() -> bool { true }

#[tauri::command]
pub async fn export_rows(req: ExportRequest) -> AppResult<usize> {
    let mut f = std::fs::File::create(&req.path)?;
    use std::io::Write;
    match req.format.as_str() {
        "csv" => {
            let mut w = csv::Writer::from_writer(&mut f);
            if req.header {
                w.write_record(&req.columns).map_err(|e| AppError::msg(e.to_string()))?;
            }
            for r in &req.rows {
                w.write_record(r.iter().map(|c| c.as_deref().unwrap_or(""))).map_err(|e| AppError::msg(e.to_string()))?;
            }
            w.flush()?;
        }
        "json" => {
            let objs: Vec<serde_json::Map<String, serde_json::Value>> = req
                .rows
                .iter()
                .map(|r| {
                    req.columns
                        .iter()
                        .zip(r.iter())
                        .map(|(c, v)| (c.clone(), v.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null)))
                        .collect()
                })
                .collect();
            f.write_all(serde_json::to_string_pretty(&objs)?.as_bytes())?;
        }
        "sql" => {
            let table = req.table.clone().unwrap_or_else(|| "table".into());
            for r in &req.rows {
                let vals: Vec<String> = r.iter().map(|v| match v { None => "NULL".into(), Some(s) => quote_literal(s) }).collect();
                writeln!(
                    f,
                    "INSERT INTO {} ({}) VALUES ({});",
                    table,
                    req.columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", "),
                    vals.join(", ")
                )?;
            }
        }
        "markdown" => {
            writeln!(f, "| {} |", req.columns.join(" | "))?;
            writeln!(f, "|{}|", req.columns.iter().map(|_| " --- ").collect::<Vec<_>>().join("|"))?;
            for r in &req.rows {
                writeln!(f, "| {} |", r.iter().map(|v| v.as_deref().unwrap_or("NULL").replace('|', "\\|")).collect::<Vec<_>>().join(" | "))?;
            }
        }
        other => return Err(AppError::msg(format!("unknown export format {other}"))),
    }
    Ok(req.rows.len())
}

#[tauri::command]
pub async fn write_text_file(path: String, content: String) -> AppResult<()> {
    std::fs::write(path, content)?;
    Ok(())
}

#[tauri::command]
pub async fn read_text_file(path: String) -> AppResult<String> {
    Ok(std::fs::read_to_string(path)?)
}
