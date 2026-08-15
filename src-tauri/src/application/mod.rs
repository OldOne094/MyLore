//! Application layer: services / use-cases.
//!
//! `media_service` (MISSION-038) is live; `tracking_service`, `search_service`,
//! `import_service`, `export_service`, `backup_service`, `provider_coordinator`,
//! `collection_service` land with their milestones (M6–M11).

pub mod activity_service;
pub mod bulk_service;
pub mod dashboard_service;
pub mod enrich_service;
pub mod import_service;
pub mod media_service;
pub mod node_service;
pub mod progress_service;
pub mod providers;
pub mod search_service;
pub mod tracking_service;
pub mod trash_service;

/// Placeholder to keep the crate skeleton compiling until M3 services land.
pub struct Placeholder;
