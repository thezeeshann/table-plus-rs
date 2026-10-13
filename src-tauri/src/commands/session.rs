use crate::error::AppResult;
use crate::model::*;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn open_session(
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
) -> AppResult<SessionInfo> {
    let cfg = state.storage.get_connection(&connection_id)?;
    let info = state.sessions.open(cfg, database).await?;
    // remember last used connection like TablePlus' "Recently opened"
    if let Ok(mut s) = state.storage.settings() {
        s.last_connection_id = Some(connection_id);
        let _ = state.storage.save_settings(&s);
    }
    Ok(info)
}

#[tauri::command]
pub async fn close_session(state: State<'_, AppState>, session_id: String) -> AppResult<()> {
    state.sessions.close(&session_id).await
}

#[tauri::command]
pub async fn reconnect_session(state: State<'_, AppState>, session_id: String) -> AppResult<SessionInfo> {
    state.sessions.reconnect(&session_id).await
}

#[tauri::command]
pub async fn cancel_query(state: State<'_, AppState>, session_id: String) -> AppResult<()> {
    state.sessions.cancel(&session_id).await
}

#[tauri::command]
pub async fn session_alive(state: State<'_, AppState>, session_id: String) -> AppResult<bool> {
    Ok(state.sessions.is_alive(&session_id).await)
}
