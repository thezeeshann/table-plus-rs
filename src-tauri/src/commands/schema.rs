use tokio_postgres::types::Type;
use crate::error::AppResult;
use crate::model::*;
use crate::pg::catalog;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_databases(state: State<'_, AppState>, session_id: String) -> AppResult<Vec<String>> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    catalog::list_databases(&s.client).await
}

#[tauri::command]
pub async fn list_schemas(state: State<'_, AppState>, session_id: String, show_system: Option<bool>) -> AppResult<Vec<String>> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    catalog::list_schemas(&s.client, show_system.unwrap_or(s.config.show_system_schemas)).await
}

#[tauri::command]
pub async fn list_schema_items(state: State<'_, AppState>, session_id: String, schema: String) -> AppResult<SchemaListing> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    catalog::list_items(&s.client, &schema, s.info.server_version_num).await
}

#[tauri::command]
pub async fn table_structure(
    state: State<'_, AppState>,
    session_id: String,
    schema: String,
    table: String,
    kind: String,
) -> AppResult<TableStructure> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    catalog::table_structure(&s.client, &schema, &table, &kind, s.info.server_version_num).await
}

#[tauri::command]
pub async fn object_ddl(
    state: State<'_, AppState>,
    session_id: String,
    schema: String,
    name: String,
    kind: String,
) -> AppResult<String> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    catalog::object_ddl(&s.client, &schema, &name, &kind, s.info.server_version_num).await
}

#[tauri::command]
pub async fn list_processes(state: State<'_, AppState>, session_id: String) -> AppResult<Vec<ProcessInfo>> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    catalog::processes(&s.client).await
}

#[tauri::command]
pub async fn kill_process(state: State<'_, AppState>, session_id: String, pid: i32, terminate: bool) -> AppResult<bool> {
    let s = state.sessions.get(&session_id).await?;
    let sql = if terminate { "SELECT pg_terminate_backend($1)" } else { "SELECT pg_cancel_backend($1)" };
    let row = s.client.query_typed_one(sql, &[(&pid, Type::INT4)]).await?;
    Ok(row.get(0))
}

/// Column names for every table in a schema; feeds editor autocompletion.
#[tauri::command]
pub async fn schema_columns(
    state: State<'_, AppState>,
    session_id: String,
    schema: String,
) -> AppResult<std::collections::BTreeMap<String, Vec<String>>> {
    let s = state.sessions.get(&session_id).await?;
    let _guard = s.exec_lock.lock().await;
    let rows = s
        .client
        .query_typed(
            "SELECT c.relname::text, a.attname::text FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1 AND a.attnum > 0 AND NOT a.attisdropped \
             AND c.relkind IN ('r','v','m','f','p') ORDER BY c.relname, a.attnum",
            &[(&schema, Type::TEXT)],
        )
        .await?;
    let mut map: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for r in rows {
        map.entry(r.get(0)).or_default().push(r.get(1));
    }
    Ok(map)
}
