pub mod commands;
pub mod error;
pub mod model;
pub mod pg;
pub mod storage;

use pg::session::SessionManager;
use storage::Storage;
use tauri::Manager;

pub struct AppState {
    pub storage: Storage,
    pub sessions: SessionManager,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let storage = Storage::new(dir)?;
            app.manage(AppState { storage, sessions: SessionManager::default() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connections::list_connections,
            commands::connections::get_connection,
            commands::connections::save_connection,
            commands::connections::delete_connection,
            commands::connections::save_group,
            commands::connections::delete_group,
            commands::connections::test_connection,
            commands::connections::get_settings,
            commands::connections::save_settings,
            commands::connections::list_favorites,
            commands::connections::save_favorite,
            commands::connections::delete_favorite,
            commands::connections::list_history,
            commands::connections::clear_history,
            commands::session::open_session,
            commands::session::close_session,
            commands::session::reconnect_session,
            commands::session::cancel_query,
            commands::session::session_alive,
            commands::schema::list_databases,
            commands::schema::list_schemas,
            commands::schema::list_schema_items,
            commands::schema::table_structure,
            commands::schema::object_ddl,
            commands::schema::list_processes,
            commands::schema::kill_process,
            commands::schema::schema_columns,
            commands::data::fetch_rows,
            commands::data::preview_changes,
            commands::data::apply_changes,
            commands::data::preview_ddl,
            commands::data::apply_ddl,
            commands::data::export_rows,
            commands::data::write_text_file,
            commands::data::read_text_file,
            commands::query::run_query,
            commands::query::execute,
            commands::theme::launch_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
