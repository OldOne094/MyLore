//! Application layer: services / use-cases.
//!
//! `media_service` (MISSION-038) is live; `tracking_service`, `search_service`,
//! `import_service`, `export_service`, `backup_service`, `provider_coordinator`
//! land with their milestones (M6–M11). `collection_service` landed with
//! MISSION-076.

pub mod activity_service;
pub mod bulk_service;
pub mod calendar_service;
pub mod collection_service;
pub mod dashboard_service;
pub mod enrich_service;
pub mod export_service;
pub mod image_service;
pub mod import_file_service;
pub mod import_pipeline;
pub mod import_service;
pub mod media_service;
pub mod node_service;
pub mod progress_service;
pub mod providers;
pub mod review_service;
pub mod search_service;
pub mod stats_service;
pub mod task_service;
pub mod tracking_service;
pub mod trash_service;

/// Placeholder to keep the crate skeleton compiling until M3 services land.
pub struct Placeholder;
