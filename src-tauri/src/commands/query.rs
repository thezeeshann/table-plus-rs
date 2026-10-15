use crate::commands::data::record_history;
use crate::error::AppResult;
use crate::model::*;
use crate::pg::query::{run_script, run_statement, MAX_RESULT_ROWS};
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunQueryRequest {
    pub session_id: String,
    pub sql: String,
    /// true = split into statements and run each (Run All); false = run as one.
    #[serde(default)]
    pub split: bool,
    #[serde(default)]
    pub row_limit: Option<usize>,
    #[serde(default = "default_true")]
    pub stop_on_error: bool,
    #[serde(default = "default_true")]
    pub record: bool,
}
fn default_true() -> bool { true }

#[tauri::command]
pub async fn run_query(state: State<'_, AppState>, req: RunQueryRequest) -> AppResult<Vec<QueryResult>> {
    let s = state.sessions.get(&req.session_id).await?;
    let _guard = s.exec_lock.lock().await;
    let limit = req.row_limit.unwrap_or(MAX_RESULT_ROWS).min(MAX_RESULT_ROWS);
    let results = if req.split {
        run_script(&s.client, &req.sql, limit, req.stop_on_error, s.supports_prepare).await?
    } else {
        run_statement(&s.client, req.sql.trim(), limit, s.supports_prepare).await?
    };
    if req.record {
        for r in &results {
            let ok = r.error.is_none();
            let msg = r.error.as_ref().and_then(|e| e.get("message")).and_then(|m| m.as_str()).map(|s| s.to_string());
            record_history(&state, &s.info, &r.statement, r.duration_ms, ok, msg, r.rows_affected);
        }
    }
    Ok(results)
}

/// Executes a statement without returning rows; used for quick actions
/// (drop/truncate/refresh matview) that only need success/failure.
#[tauri::command]
pub async fn execute(state: State<'_, AppState>, session_id: String, sql: String) -> AppResult<u64> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    let results = run_statement(&s.client, sql.trim(), 1, false).await?;
    let mut affected = 0u64;
    for r in results {
        if let Some(e) = r.error {
            record_history(&state, &s.info, &sql, r.duration_ms, false, e.get("message").and_then(|m| m.as_str()).map(String::from), None);
            return Err(crate::error::AppError::Message(
                e.get("message").and_then(|m| m.as_str()).unwrap_or("query failed").to_string(),
            ));
        }
        affected += r.rows_affected.unwrap_or(0);
        record_history(&state, &s.info, &sql, r.duration_ms, true, None, r.rows_affected);
    }
    Ok(affected)
}
