//! Activity-log writes (MISSION-051).
//!
//! Append-only user events for every tracking action: status transitions
//! (started / completed / repeat) and per-node progress marks. Writes are
//! **best-effort** — a failed log must never fail the user's action, so errors
//! are logged and swallowed. Consumers (calendar, stats, undo) land in later
//! missions; this module only guarantees the writes happen.

use chrono::Utc;
use sqlx::SqlitePool;

use crate::domain::enums::CoreStatus;
use crate::infrastructure::repositories::activity::{self, ActivityRecord};

/// Best-effort append of a tracking activity entry.
pub async fn log_tracking(
    pool: &SqlitePool,
    media_id: &str,
    node_id: Option<&str>,
    kind: &str,
    meta: &serde_json::Value,
) {
    let record = ActivityRecord {
        id: format!("a-{}", uuid::Uuid::new_v4()),
        media_id: Some(media_id.to_string()),
        node_id: node_id.map(str::to_string),
        kind: kind.to_string(),
        meta: Some(meta.to_string()),
        created_at: Utc::now().to_rfc3339(),
    };
    if let Err(err) = activity::log(pool, &record).await {
        tracing::warn!(%err, media_id, kind, "activity log write failed");
    }
}

/// Log a status transition. Only statuses with a matching activity kind produce
/// an entry (`started` / `completed` / `repeat`); manual/planned transitions
/// (on_hold, dropped, wishlist, planned) have no kind and write nothing.
pub async fn log_status_transition(
    pool: &SqlitePool,
    media_id: &str,
    from: &CoreStatus,
    to: &CoreStatus,
) {
    let kind = match to {
        CoreStatus::InProgress => "started",
        CoreStatus::Completed => "completed",
        CoreStatus::Repeat => "repeat",
        _ => return,
    };
    let meta = serde_json::json!({ "from": from.as_str(), "to": to.as_str() });
    log_tracking(pool, media_id, None, kind, &meta).await;
}

/// Log a per-node progress mark (`progress`).
pub async fn log_progress(pool: &SqlitePool, media_id: &str, node_id: &str, state: &str) {
    let meta = serde_json::json!({ "state": state });
    log_tracking(pool, media_id, Some(node_id), "progress", &meta).await;
}
