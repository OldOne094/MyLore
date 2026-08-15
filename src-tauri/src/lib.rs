//! MyLore — Tauri application (library crate).
//!
//! Layers (spec §83): `commands` (thin IPC) → `application` (services) →
//! `domain` (pure) ⇐ `infrastructure` (sqlx repos, providers, fs, keyring).

pub mod application;
pub mod commands;
pub mod domain;
pub mod error;
pub mod infrastructure;

use std::sync::Arc;

use tauri::Manager;

use crate::application::providers::coordinator::ProviderCoordinator;

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

            let coordinator = Arc::new(
                ProviderCoordinator::new(infrastructure::providers::default_provider_entries())
                    .map_err(std::io::Error::other)?,
            );
            app.manage(coordinator);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::media::media_create,
            commands::media::media_list,
            commands::media::media_facets,
            commands::media::media_get,
            commands::media::media_search,
            commands::discover::search_external,
            commands::import::import_provider,
            commands::dashboard::dashboard_summary,
            commands::node::media_nodes,
            commands::node::node_progress_set,
            commands::node::node_progress_range,
            commands::node::node_progress_next,
            commands::trash::media_delete,
            commands::trash::trash_list,
            commands::trash::trash_restore,
            commands::trash::trash_purge,
            commands::bulk::tracking_bulk_set_status,
            commands::bulk::media_bulk_add_tag,
            commands::bulk::media_bulk_delete,
            commands::bulk::collection_list,
            commands::bulk::collection_bulk_add,
            commands::tracking::tracking_get,
            commands::tracking::tracking_set_status,
            commands::tracking::tracking_set_auto_track
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
