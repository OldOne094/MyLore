//! Library statistics service (MISSION-080).
//!
//! Wires the pure MISSION-027 computations to the database: loads one
//! projection row per tracked media (status, content type, rating, favorite,
//! release year) plus the batched per-media progress numbers, folds them into
//! `MediaStatsRow`s and resolves a serializable `StatsView` for the Stats page.
//!
//! Time is reported from real data only: `consumed_minutes`/`consumed_hours`
//! sum node-level completed minutes; reading progress reports pages. Node-tree
//! estimates (`with_estimate`) are deliberately not used here — the stats are
//! about what the user actually consumed.

use std::collections::HashMap;
use std::str::FromStr;

use sqlx::SqlitePool;

use crate::domain::enums::{ContentType, CoreStatus};
use crate::domain::progress::{ProgressAggregate, ProgressTemplate};
use crate::domain::stats::{compute_stats, MediaStatsRow, StatsSummary};
use crate::domain::value_objects::{MediaId, Rating};
use crate::error::AppError;
use crate::infrastructure::repositories::tracking::{self, ProgressStatsRow};

/// A labelled bucket count for the Stats page charts/tables. `key` is the enum
/// string (a `coreStatus.*`/`contentType.*` i18n key) or a numeric value as
/// text (rating value / release year); the frontend renders the label.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct StatCount {
    pub key: String,
    pub count: u64,
}

/// The serializable stats overview (MISSION-080).
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatsView {
    /// Number of tracked media.
    pub total: u64,
    /// Count per core status, in schema order.
    pub status_counts: Vec<StatCount>,
    /// Count per content type, in schema order.
    pub content_type_counts: Vec<StatCount>,
    /// Count per rating value 1..=10.
    pub rating_counts: Vec<StatCount>,
    /// Mean of non-null user ratings.
    pub avg_rating: Option<f64>,
    /// Media marked as favorite.
    pub favorites: u64,
    /// Media in the completed bucket.
    pub completed_media: u64,
    /// `completed_media / total` (None when nothing is tracked).
    pub completion_rate: Option<f64>,
    /// Mean aggregate percent over media that have one (None when none do).
    pub avg_percent: Option<f64>,
    /// Sum of node-level completed minutes (episode durations).
    pub consumed_minutes: u64,
    /// `consumed_minutes` expressed in hours.
    pub consumed_hours: f64,
    /// Sum of completed pages (book-style aggregates).
    pub consumed_pages: u64,
    /// Count per release year (ascending).
    pub year_counts: Vec<StatCount>,
}

impl StatsView {
    fn from_summary(summary: &StatsSummary) -> Self {
        Self {
            total: summary.total,
            status_counts: count_view(&summary.status_counts),
            content_type_counts: count_view(&summary.content_type_counts),
            rating_counts: summary
                .rating_counts
                .iter()
                .map(|(key, count)| StatCount {
                    key: key.to_string(),
                    count: *count,
                })
                .collect(),
            avg_rating: summary.avg_rating,
            favorites: summary.favorites,
            completed_media: summary.completed_media,
            completion_rate: summary.completion_rate,
            avg_percent: summary.avg_percent,
            consumed_minutes: summary.consumed_minutes,
            consumed_hours: summary.consumed_hours(),
            consumed_pages: summary.consumed_pages,
            year_counts: summary
                .year_counts
                .iter()
                .map(|(key, count)| StatCount {
                    key: key.to_string(),
                    count: *count,
                })
                .collect(),
        }
    }
}

/// Map an enum-keyed distribution onto the string-keyed DTO.
fn count_view<T: AsStr>(counts: &[(T, u64)]) -> Vec<StatCount> {
    counts
        .iter()
        .map(|(key, count)| StatCount {
            key: key.as_str().to_string(),
            count: *count,
        })
        .collect()
}

/// Local shim so both enum distributions share one mapper.
trait AsStr {
    fn as_str(&self) -> &'static str;
}
impl AsStr for CoreStatus {
    fn as_str(&self) -> &'static str {
        (*self).as_str()
    }
}
impl AsStr for ContentType {
    fn as_str(&self) -> &'static str {
        (*self).as_str()
    }
}

/// Library statistics use-cases.
pub struct StatsService {
    pool: SqlitePool,
}

impl StatsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Resolve the whole-library statistics overview. Untracked media are not
    /// part of the stats (there is nothing to measure yet).
    pub async fn summary(&self) -> Result<StatsView, AppError> {
        let tracked = tracking::tracked_media(&self.pool).await?;
        let ids: Vec<String> = tracked.iter().map(|row| row.media_id.clone()).collect();
        let progress = tracking::progress_stats(&self.pool, &ids).await?;
        let progress_by_media: HashMap<String, &ProgressStatsRow> = progress
            .iter()
            .map(|row| (row.media_id.clone(), row))
            .collect();

        let mut media_rows = Vec::with_capacity(tracked.len());
        for row in tracked {
            let content_type = ContentType::from_str(&row.content_type)?;
            let core_status = CoreStatus::from_str(&row.core_status)?;
            let progress = progress_by_media.get(&row.media_id).copied();
            media_rows.push(MediaStatsRow {
                media_id: MediaId::new(&row.media_id)?,
                content_type,
                core_status,
                rating: row.rating.and_then(|r| Rating::new(r).ok()),
                favorite: row.favorite,
                release_year: row.release_year.and_then(|y| u16::try_from(y).ok()),
                progress: aggregate_for(content_type, progress),
            });
        }

        Ok(StatsView::from_summary(&compute_stats(&media_rows)))
    }
}

/// Fold a batched progress row into the domain aggregate (weights match the
/// progress engine; only consumed minutes are reported).
fn aggregate_for(content_type: ContentType, row: Option<&ProgressStatsRow>) -> ProgressAggregate {
    let template = ProgressTemplate::for_content_type(content_type);
    let (total, completed, minutes) = match row {
        Some(row) => (
            row.total_weight.max(0) as u64,
            row.completed_weight.max(0) as u64,
            row.completed_minutes.max(0) as u64,
        ),
        None => (0, 0, 0),
    };
    ProgressAggregate {
        template,
        total_units: total,
        completed_units: completed,
        percent: (total > 0).then(|| (completed.saturating_mul(100) / total) as u8),
        total_minutes: None,
        completed_minutes: (minutes > 0).then_some(minutes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::repositories::{media, node, tracking};
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn seed_media(pool: &sqlx::SqlitePool, id: &str, content_type: &str, year: Option<i64>) {
        media::create(
            pool,
            &media::MediaRecord {
                id: id.to_string(),
                content_type: content_type.into(),
                format: None,
                title_main: format!("Title {id}"),
                title_original: None,
                synopsis: None,
                pub_status: "unknown".into(),
                start_date: None,
                end_date: None,
                release_year: year,
                language: None,
                country: None,
                content_rating: None,
                pages: None,
                duration_min: None,
                ep_count: None,
                ch_count: None,
                cover_asset_id: None,
                banner_asset_id: None,
                provider: None,
                provider_url: None,
                metadata_refreshed_at: None,
                created_at: "2026-01-01".into(),
                updated_at: "2026-01-01".into(),
                alt_titles: Vec::new(),
                people: Vec::new(),
                genres: Vec::new(),
                tags: Vec::new(),
                external_ids: Vec::new(),
                relations: Vec::new(),
            },
        )
        .await
        .expect("seed media");
    }

    async fn seed_node(
        pool: &sqlx::SqlitePool,
        id: &str,
        media_id: &str,
        kind: &str,
        position: i64,
        duration_min: Option<i64>,
    ) {
        node::create(
            pool,
            &node::NodeRecord {
                id: id.to_string(),
                media_id: media_id.to_string(),
                parent_id: None,
                kind: kind.into(),
                position,
                number: Some(format!("n{position}")),
                title: None,
                release_date: None,
                duration_min,
                page_count: None,
                synopsis: None,
                external_id: None,
                is_special: false,
                created_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("seed node");
    }

    async fn track(pool: &sqlx::SqlitePool, media_id: &str, status: &str) {
        tracking::upsert_tracking(
            pool,
            &tracking::TrackingRecord {
                media_id: media_id.to_string(),
                core_status: status.into(),
                custom_status_id: None,
                started_at: None,
                finished_at: None,
                repeat_count: 0,
                current_node_id: None,
                current_position: None,
                auto_track: 1,
                updated_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("track");
    }

    async fn mark(pool: &sqlx::SqlitePool, node_id: &str, state: &str) {
        tracking::set_progress(
            pool,
            &tracking::NodeProgress {
                node_id: node_id.into(),
                state: state.into(),
                read_at: None,
                note: None,
                rating: None,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("mark");
    }

    #[tokio::test]
    async fn summary_computes_counts_rating_and_consumption() {
        let (pool, path) = migrated_pool("stats_service_summary.db").await;
        seed_media(&pool, "m-1", "anime", Some(2011)).await;
        seed_media(&pool, "m-2", "book", Some(2011)).await;
        seed_media(&pool, "m-3", "novel", None).await;
        seed_node(&pool, "e1", "m-1", "episode", 1, Some(24)).await;
        seed_node(&pool, "e2", "m-1", "episode", 2, Some(24)).await;
        seed_node(&pool, "c1", "m-2", "chapter", 1, None).await;
        seed_node(&pool, "c2", "m-2", "chapter", 2, None).await;
        track(&pool, "m-1", "completed").await;
        track(&pool, "m-2", "in_progress").await;
        track(&pool, "m-3", "planned").await;
        mark(&pool, "e1", "watched").await;
        mark(&pool, "e2", "watched").await;
        mark(&pool, "c1", "read").await;
        sqlx::query(
            "INSERT INTO review (media_id, rating, favorite, created_at, updated_at)
                     VALUES ('m-1', 9, 1, '2026-01-02', '2026-01-02')",
        )
        .execute(&pool)
        .await
        .expect("seed review");

        let view = StatsService::new(pool.clone())
            .summary()
            .await
            .expect("summary");

        assert_eq!(view.total, 3);
        assert_eq!(view.completed_media, 1);
        assert_eq!(view.completion_rate, Some(1.0 / 3.0));
        assert_eq!(view.avg_rating, Some(9.0));
        assert_eq!(view.favorites, 1);
        // 2 × 24-min episodes watched.
        assert_eq!(view.consumed_minutes, 48);
        assert_eq!(view.consumed_hours, 0.8);
        assert_eq!(
            view.consumed_pages, 1,
            "the read book chapter counts 1 page (no page_count)"
        );

        let completed = view
            .status_counts
            .iter()
            .find(|s| s.key == "completed")
            .expect("completed bucket")
            .count;
        assert_eq!(completed, 1);
        let anime = view
            .content_type_counts
            .iter()
            .find(|t| t.key == "anime")
            .expect("anime bucket")
            .count;
        assert_eq!(anime, 1);
        let rating = view
            .rating_counts
            .iter()
            .find(|r| r.key == "9")
            .expect("rating 9")
            .count;
        assert_eq!(rating, 1);
        assert_eq!(
            view.year_counts,
            vec![StatCount {
                key: "2011".into(),
                count: 2
            }]
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn summary_weighs_book_pages_as_consumed() {
        let (pool, path) = migrated_pool("stats_service_pages.db").await;
        seed_media(&pool, "m-1", "book", None).await;
        node::create(
            &pool,
            &node::NodeRecord {
                id: "c1".into(),
                media_id: "m-1".into(),
                parent_id: None,
                kind: "chapter".into(),
                position: 1,
                number: None,
                title: None,
                release_date: None,
                duration_min: None,
                page_count: Some(120),
                synopsis: None,
                external_id: None,
                is_special: false,
                created_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("seed chapter");
        track(&pool, "m-1", "in_progress").await;
        mark(&pool, "c1", "read").await;

        let view = StatsService::new(pool.clone())
            .summary()
            .await
            .expect("summary");
        assert_eq!(view.consumed_pages, 120);
        assert_eq!(view.consumed_minutes, 0);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn summary_empty_library_is_zeroed() {
        let (pool, path) = migrated_pool("stats_service_empty.db").await;
        let view = StatsService::new(pool.clone())
            .summary()
            .await
            .expect("summary");
        assert_eq!(view.total, 0);
        assert_eq!(view.completion_rate, None);
        assert_eq!(view.avg_rating, None);
        assert_eq!(view.consumed_hours, 0.0);
        assert!(view.status_counts.iter().all(|s| s.count == 0));
        assert!(view.year_counts.is_empty());
        pool.close().await;
        cleanup_files(&path);
    }
}
