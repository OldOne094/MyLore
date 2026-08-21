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

use crate::application::app_health::AppHealth;
use crate::application::backup_service::BackupService;
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
            // Startup timing (MISSION-094): the database phase is the bulk of
            // pre-window work; log its duration so regressions surface.
            let startup = std::time::Instant::now();
            let data_dir = app.path().app_data_dir()?;
            infrastructure::logging::init(&data_dir.join("logs"));

            let db_path = data_dir.join("mylore.db");

            // Pre-migration safety backup (MISSION-087): when the schema is
            // about to move forward, snapshot the old database first. Best
            // effort — a failed backup logs a warning and startup continues.
            match tauri::async_runtime::block_on(BackupService::pre_migration_backup(&db_path)) {
                Ok(Some(report)) => {
                    tracing::info!(path = %report.path, "pre-migration backup created")
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "pre-migration backup failed; continuing"),
            }

            let pool = tauri::async_runtime::block_on(infrastructure::db::connect(&db_path))?;
            tracing::info!(
                db = %db_path.display(),
                ms = startup.elapsed().as_millis() as u64,
                "database opened"
            );

            // MISSION-088: verify integrity before migrating. On corruption
            // the app still launches — in recovery mode — so the recovery
            // screen can offer a restore or a fresh start instead of the
            // process dying before any window shows.
            let database_ok =
                tauri::async_runtime::block_on(infrastructure::db::integrity_check(&pool)).is_ok();
            if database_ok {
                if let Err(error) =
                    tauri::async_runtime::block_on(infrastructure::db::migrate(&pool))
                {
                    return Err(error.into());
                }
            } else {
                tracing::error!("database failed its integrity check; starting in recovery mode");
            }
            app.manage(pool.clone());
            app.manage(Arc::new(AppHealth::new(database_ok)));

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
            app.manage(Arc::new(ImageService::new(pool.clone(), &images_dir)));

            // Backup service (MISSION-084): archives under
            // `{data_dir}/backups`, cached assets from `{data_dir}/images`.
            let backups = Arc::new(BackupService::new(pool, &data_dir));

            // Automatic backup check (MISSION-086): shortly after startup,
            // create a backup when the preference is on and the newest
            // archive is older than the configured interval.
            let auto_backups = backups.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                match auto_backups.auto_backup_if_due().await {
                    Ok(Some(report)) => {
                        tracing::info!(path = %report.path, "automatic backup created")
                    }
                    Ok(None) => {}
                    Err(error) => tracing::warn!(%error, "automatic backup failed"),
                }
            });
            app.manage(backups);

            tracing::info!(
                ms = startup.elapsed().as_millis() as u64,
                "startup services ready"
            );

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
            commands::backup::backup_create,
            commands::backup::backup_validate,
            commands::backup::backup_restore,
            commands::backup::backup_prefs_get,
            commands::backup::backup_prefs_set,
            commands::backup::backup_list,
            commands::backup::backup_delete,
            commands::recovery::app_health,
            commands::recovery::recover_start_fresh,
            commands::recovery::recover_restore,
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
            commands::collection::collection_create_smart,
            commands::collection::collection_update_smart,
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
            commands::review::review_acknowledge_warnings,
            commands::review::review_delete,
            commands::stats::stats_summary,
            commands::calendar::calendar_month,
            commands::recap::recap_year,
            commands::reading::reading_recap,
            commands::media::media_tags,
            commands::media::media_add_tag,
            commands::media::media_remove_tag,
            commands::merge::merge_plan,
            commands::merge::merge_apply
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
