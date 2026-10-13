use crate::error::AppResult;
use crate::model::*;
use crate::pg::session::SessionManager;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_connections(state: State<'_, AppState>) -> AppResult<(Vec<ConnectionConfig>, Vec<ConnectionGroup>)> {
    state.storage.list_connections()
}

#[tauri::command]
pub async fn get_connection(state: State<'_, AppState>, id: String) -> AppResult<ConnectionConfig> {
    state.storage.get_connection(&id)
}

#[tauri::command]
pub async fn save_connection(state: State<'_, AppState>, config: ConnectionConfig) -> AppResult<ConnectionConfig> {
    state.storage.save_connection(config)
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.storage.delete_connection(&id)
}

#[tauri::command]
pub async fn save_group(state: State<'_, AppState>, group: ConnectionGroup) -> AppResult<ConnectionGroup> {
    state.storage.save_group(group)
}

#[tauri::command]
pub async fn delete_group(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.storage.delete_group(&id)
}

/// Test with the password from the form, or the stored one if the form left it blank.
#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>, mut config: ConnectionConfig) -> AppResult<String> {
    if config.password.as_deref().unwrap_or("").is_empty() && !config.id.is_empty() {
        if let Ok(stored) = state.storage.get_connection(&config.id) {
            config.password = stored.password;
        }
    }
    SessionManager::test(&config).await
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.storage.settings()
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> AppResult<()> {
    state.storage.save_settings(&settings)
}

#[tauri::command]
pub async fn list_favorites(state: State<'_, AppState>) -> AppResult<Vec<FavoriteQuery>> {
    state.storage.favorites()
}

#[tauri::command]
pub async fn save_favorite(state: State<'_, AppState>, favorite: FavoriteQuery) -> AppResult<FavoriteQuery> {
    state.storage.save_favorite(favorite)
}

#[tauri::command]
pub async fn delete_favorite(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.storage.delete_favorite(&id)
}

#[tauri::command]
pub async fn list_history(
    state: State<'_, AppState>,
    connection_id: String,
    limit: Option<usize>,
    search: Option<String>,
) -> AppResult<Vec<HistoryEntry>> {
    state.storage.history(&connection_id, limit.unwrap_or(500), search.as_deref())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>, connection_id: String) -> AppResult<()> {
    state.storage.clear_history(&connection_id)
}
