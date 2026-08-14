//! Dashboard application service (MISSION-050).
//!
//! Aggregates the widget lists for the dashboard home: "Continue reading /
//! watching" (active tracking, most recently updated), "Recently completed"
//! (newest finish first), and "Recently added" (newest `created_at` first).
//! Every widget maps through the shared `MediaListItem` DTO so the rows carry
//! their progress summaries (and with them the in-grid quick controls).

use sqlx::SqlitePool;

use crate::application::media_service::{MediaListItem, MediaService};
use crate::error::AppError;
use crate::infrastructure::repositories::media::{MediaFilter, MediaSort};
use crate::infrastructure::repositories::tracking::{recent_media_by_status, RecentOrder};

/// All dashboard widget payloads in one round-trip.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DashboardSummary {
    pub continue_watching: Vec<MediaListItem>,
    pub recently_completed: Vec<MediaListItem>,
    pub recently_added: Vec<MediaListItem>,
}

/// Dashboard use-cases.
pub struct DashboardService {
    pool: SqlitePool,
}

impl DashboardService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Resolve the widget lists. `limit` is clamped to 1..=20 per widget.
    pub async fn summary(&self, limit: Option<u32>) -> Result<DashboardSummary, AppError> {
        let limit = limit.unwrap_or(8).clamp(1, 20) as i64;
        let media = MediaService::new(self.pool.clone());

        let continue_rows = recent_media_by_status(
            &self.pool,
            &["in_progress", "repeat"],
            RecentOrder::UpdatedAt,
            limit,
        )
        .await?;
        let completed_rows =
            recent_media_by_status(&self.pool, &["completed"], RecentOrder::FinishedAt, limit)
                .await?;
        let added_rows = crate::infrastructure::repositories::media::list(
            &self.pool,
            &MediaFilter {
                sort: MediaSort::CreatedAt,
                ascending: false,
                limit: Some(limit as u32),
                ..MediaFilter::default()
            },
        )
        .await?;

        Ok(DashboardSummary {
            continue_watching: media.to_list_items(continue_rows).await?,
            recently_completed: media.to_list_items(completed_rows).await?,
            recently_added: media.to_list_items(added_rows).await?,
        })
    }
}
