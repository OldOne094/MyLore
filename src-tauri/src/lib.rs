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

use tauri::Emitter;
use tauri::Manager;

use crate::application::image_service::ImageService;
use crate::application::providers::settings::ProviderSettingsService;
use crate::application::task_service::TaskManager;
use crate::infrastructure::keyring::OsKeyring;
use crate::infrastructure::providers::StdEntryBuilder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            infrastructure::logging::init(&data_dir.join("logs"));

            let db_path = data_dir.join("mylore.db");
            let pool = tauri::async_runtime::block_on(infrastructure::db::init(&db_path))?;
            tracing::info!(db = %db_path.display(), "database opened");
            app.manage(pool.clone());

            // Provider set: default configs, layered with persisted enabled
            // flags + keyring keys. Rebuilt on settings changes (MISSION-063).
            let settings = Arc::new(
                ProviderSettingsService::load(
                    infrastructure::providers::default_provider_configs(),
                    data_dir.join("providers.json"),
                    Box::new(OsKeyring),
                    Arc::new(StdEntryBuilder),
                )
                .map_err(std::io::Error::other)?,
            );
            app.manage(settings);

            let images_dir = data_dir.join("images");
            app.manage(Arc::new(ImageService::new(pool, &images_dir)));

            // Background task manager (MISSION-070): every long operation runs
            // as a cancelable task; changes stream to the UI as `task_changed`.
            let task_manager = TaskManager::with_emitter({
                let handle = app.handle().clone();
                move |snapshot| {
                    let _ = handle.emit("task-changed", snapshot);
                }
            });
            app.manage(Arc::new(task_manager));

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
            commands::import::import_file_preview,
            commands::import::import_file_detect,
            commands::import::import_commit,
            commands::import::import_csv_headers,
            commands::enrich::media_enrich,
            commands::export::export_media,
            commands::providers::providers_list,
            commands::providers::provider_set_enabled,
            commands::providers::provider_set_key,
            commands::providers::provider_test_connection,
            commands::images::asset_resolve,
            commands::images::assets_resolve,
            commands::dashboard::dashboard_summary,
            commands::node::media_nodes,
            commands::node::node_progress_set,
            commands::node::node_progress_range,
            commands::node::node_progress_next,
            commands::trash::media_delete,
            commands::trash::trash_list,
            commands::trash::trash_restore,
            commands::trash::trash_purge,
            commands::tasks::task_list,
            commands::tasks::task_get,
            commands::tasks::task_cancel,
            commands::bulk::tracking_bulk_set_status,
            commands::bulk::media_bulk_add_tag,
            commands::bulk::media_bulk_delete,
            commands::collection::collection_list,
            commands::collection::collection_create,
            commands::collection::collection_rename,
            commands::collection::collection_delete,
            commands::collection::collection_members,
            commands::collection::collection_bulk_add,
            commands::collection::collection_remove_member,
            commands::collection::collection_reorder,
            commands::tracking::tracking_get,
            commands::tracking::tracking_set_status,
            commands::tracking::tracking_set_auto_track,
            commands::review::review_get,
            commands::review::review_save,
            commands::review::review_delete,
            commands::media::media_tags,
            commands::media::media_add_tag,
            commands::media::media_remove_tag
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
