//! MyLore — Tauri application (library crate).
//!
//! Layers (spec §83): `commands` (thin IPC) → `application` (services) →
//! `domain` (pure) ⇐ `infrastructure` (sqlx repos, providers, fs, keyring).

pub mod application;
pub mod commands;
pub mod domain;
pub mod error;
pub mod infrastructure;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            infrastructure::logging::init(&data_dir.join("logs"));

            let db_path = data_dir.join("mylore.db");
            let pool = tauri::async_runtime::block_on(infrastructure::db::init(&db_path))?;
            tracing::info!(db = %db_path.display(), "database opened");
            app.manage(pool);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::media::media_create,
            commands::media::media_list,
            commands::media::media_facets,
            commands::media::media_get,
            commands::media::media_search,
            commands::trash::media_delete,
            commands::trash::trash_list,
            commands::trash::trash_restore,
            commands::trash::trash_purge
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
