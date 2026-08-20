//! Year-in-review service (MISSION-082).
//!
//! A celebratory, whole-year recap assembled from the user activity trail
//! (MISSION-051): how many titles were added / started / completed / reviewed,
//! how much progress was logged, a monthly completion chart, the top genres of
//! the media finished, the most-active media, and the longest streak of
//! consecutive active days. Like the calendar service, events are bucketed by
//! *local* time — each RFC3339 timestamp is converted to the user's timezone
//! and only events whose local date falls inside the requested year count.

use std::collections::{BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate};
use serde::Serialize;
use sqlx::sqlite::SqlitePool;

use crate::error::AppError;
use crate::infrastructure::repositories::calendar::activity_in_range;
use crate::infrastructure::repositories::recap::completed_genres;

/// Headline counts for the year.
#[derive(Debug, Clone, Serialize)]
pub struct RecapTotals {
    /// Distinct media added to the library.
    pub added: u32,
    /// Distinct media marked as started.
    pub started: u32,
    /// Distinct media finished.
    pub completed: u32,
    /// Distinct media reviewed.
    pub reviewed: u32,
    /// Total progress events logged.
    pub progress: u32,
}

/// A genre ranked by how many distinct finished media carry it.
#[derive(Debug, Clone, Serialize)]
pub struct GenreCount {
    pub name: String,
    pub count: u32,
}

/// One of the year's most-active media.
#[derive(Debug, Clone, Serialize)]
pub struct RecapMedia {
    pub media_id: Option<String>,
    pub title: String,
    pub content_type: Option<String>,
    /// Total activity events touching this media in the year.
    pub activity_count: u32,
}

/// The whole year in review.
#[derive(Debug, Clone, Serialize)]
pub struct YearRecap {
    pub year: u16,
    pub totals: RecapTotals,
    /// Completed titles per month (12 entries, index = month − 1).
    pub by_month: Vec<u32>,
    /// Top five genres of the media finished in the year (empty when none).
    pub top_genres: Vec<GenreCount>,
    /// Top five media by activity count (empty when none).
    pub top_media: Vec<RecapMedia>,
    /// Longest run of consecutive active days (0 when inactive).
    pub longest_streak: u32,
    /// Month (1–12) with the most completions; None when nothing was completed.
    pub best_month: Option<u8>,
}

/// Backend for the year-in-review recap.
pub struct RecapService {
    pool: SqlitePool,
}

impl RecapService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Resolve the year-in-review for one year.
    pub async fn year(&self, year: u16) -> Result<YearRecap, AppError> {
        if !(1900..=2100).contains(&year) {
            return Err(AppError::validation(format!("year out of range: {year}")));
        }

        let start = NaiveDate::from_ymd_opt(year as i32, 1, 1).expect("valid year");
        let next = NaiveDate::from_ymd_opt(year as i32 + 1, 1, 1).expect("valid next year");
        // Query a window wide enough for any local offset (±1 day) and filter by
        // each event's local year below — mirrors the calendar service.
        let lo = (start - Days::new(1)).format("%Y-%m-%d").to_string();
        let hi = (next + Days::new(1)).format("%Y-%m-%d").to_string();

        let events = activity_in_range(&self.pool, &lo, &hi).await?;

        let mut added = HashSet::new();
        let mut started = HashSet::new();
        let mut completed_ids = HashSet::new();
        let mut reviewed = HashSet::new();
        let mut progress = 0u32;
        let mut by_month = vec![0u32; 12];
        let mut active_days: BTreeSet<NaiveDate> = BTreeSet::new();
        let mut per_media: HashMap<String, (String, Option<String>, u32)> = HashMap::new();

        for row in events {
            let Ok(dt) = DateTime::parse_from_rfc3339(&row.created_at) else {
                continue;
            };
            let local = dt.with_timezone(&Local);
            if local.year() != year as i32 {
                continue;
            }
            match row.kind.as_str() {
                "added" => {
                    if let Some(id) = row.media_id.as_deref() {
                        added.insert(id.to_string());
                    }
                }
                "started" => {
                    if let Some(id) = row.media_id.as_deref() {
                        started.insert(id.to_string());
                    }
                }
                "completed" => {
                    by_month[local.month() as usize - 1] += 1;
                    if let Some(id) = row.media_id.as_deref() {
                        completed_ids.insert(id.to_string());
                    }
                }
                "reviewed" => {
                    if let Some(id) = row.media_id.as_deref() {
                        reviewed.insert(id.to_string());
                    }
                }
                "progress" => progress += 1,
                _ => {}
            }
            if let Some(id) = row.media_id.as_deref() {
                let entry = per_media.entry(id.to_string()).or_insert_with(|| {
                    (
                        row.title.clone().unwrap_or_default(),
                        row.content_type.clone(),
                        0,
                    )
                });
                entry.2 += 1;
            }
            active_days.insert(local.date_naive());
        }

        let totals = RecapTotals {
            added: added.len() as u32,
            started: started.len() as u32,
            completed: completed_ids.len() as u32,
            reviewed: reviewed.len() as u32,
            progress,
        };

        let mut top_media: Vec<RecapMedia> = per_media
            .into_iter()
            .map(
                |(media_id, (title, content_type, activity_count))| RecapMedia {
                    media_id: Some(media_id),
                    title,
                    content_type,
                    activity_count,
                },
            )
            .collect();
        top_media.sort_by(|a, b| {
            b.activity_count
                .cmp(&a.activity_count)
                .then_with(|| a.title.cmp(&b.title))
        });
        top_media.truncate(5);

        let longest_streak = longest_consecutive(&active_days);
        let best_month = by_month
            .iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .max_by_key(|(_, &count)| count)
            .map(|(index, _)| index as u8 + 1);

        let ids: Vec<String> = completed_ids.into_iter().collect();
        let top_genres = completed_genres(&self.pool, &ids)
            .await?
            .into_iter()
            .map(|row| GenreCount {
                name: row.name,
                count: row.count,
            })
            .collect();

        Ok(YearRecap {
            year,
            totals,
            by_month,
            top_genres,
            top_media,
            longest_streak,
            best_month,
        })
    }
}

/// Longest run of consecutive days in a sorted set of dates.
fn longest_consecutive(days: &BTreeSet<NaiveDate>) -> u32 {
    let mut best = 0u32;
    let mut run = 0u32;
    let mut prev: Option<NaiveDate> = None;
    for &day in days {
        match prev {
            Some(p) if day.signed_duration_since(p).num_days() == 1 => run += 1,
            Some(_) => run = 1,
            None => run = 1,
        }
        best = best.max(run);
        prev = Some(day);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};
    use chrono::{TimeZone, Utc};

    async fn seed_media(pool: &SqlitePool, id: &str, content_type: &str, title: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, ?, ?, '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .bind(content_type)
        .bind(title)
        .execute(pool)
        .await
        .expect("seed media");
    }

    async fn seed_activity(pool: &SqlitePool, id: &str, media_id: &str, kind: &str, at: &str) {
        sqlx::query("INSERT INTO activity (id, media_id, kind, created_at) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(media_id)
            .bind(kind)
            .bind(at)
            .execute(pool)
            .await
            .expect("seed activity");
    }

    /// An RFC3339 UTC timestamp for the given local date/time, so bucketing
    /// tests are independent of the machine's timezone.
    fn at_local(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> String {
        Local
            .with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .expect("valid local time")
            .with_timezone(&Utc)
            .to_rfc3339()
    }

    #[tokio::test]
    async fn year_assembles_totals_chart_and_standouts() {
        let (pool, path) = migrated_pool("recap_service.db").await;
        let service = RecapService::new(pool.clone());

        seed_media(&pool, "m-1", "anime", "Series").await;
        seed_media(&pool, "m-2", "novel", "Book").await;
        seed_activity(&pool, "a-1", "m-1", "added", &at_local(2026, 1, 10, 9, 0)).await;
        seed_activity(
            &pool,
            "a-2",
            "m-1",
            "started",
            &at_local(2026, 3, 15, 12, 0),
        )
        .await;
        seed_activity(
            &pool,
            "a-3",
            "m-1",
            "completed",
            &at_local(2026, 6, 20, 18, 0),
        )
        .await;
        seed_activity(
            &pool,
            "a-4",
            "m-2",
            "completed",
            &at_local(2026, 6, 22, 19, 0),
        )
        .await;
        seed_activity(
            &pool,
            "a-5",
            "m-1",
            "progress",
            &at_local(2026, 6, 23, 20, 0),
        )
        .await;
        seed_activity(
            &pool,
            "a-6",
            "m-1",
            "progress",
            &at_local(2026, 7, 5, 21, 0),
        )
        .await;
        seed_activity(
            &pool,
            "a-7",
            "m-2",
            "reviewed",
            &at_local(2026, 9, 1, 10, 0),
        )
        .await;
        seed_activity(&pool, "a-8", "m-2", "added", &at_local(2026, 11, 11, 11, 0)).await;

        let recap = service.year(2026).await.expect("recap");

        assert_eq!(recap.totals.added, 2);
        assert_eq!(recap.totals.started, 1);
        assert_eq!(recap.totals.completed, 2);
        assert_eq!(recap.totals.reviewed, 1);
        assert_eq!(recap.totals.progress, 2);

        assert_eq!(recap.by_month.len(), 12);
        assert_eq!(recap.by_month[0], 0, "no completions in January");
        assert_eq!(recap.by_month[5], 2, "both completions land in June");
        assert_eq!(recap.by_month[6], 0);
        assert_eq!(recap.best_month, Some(6));

        assert_eq!(recap.top_media.len(), 2);
        assert_eq!(recap.top_media[0].title, "Series");
        assert_eq!(recap.top_media[0].activity_count, 5);
        assert_eq!(recap.top_media[1].title, "Book");
        assert_eq!(recap.top_media[1].activity_count, 3);

        // Active days: Jan 10, Mar 15, Jun 20, Jun 22, Jun 23, Jul 5, Sep 1,
        // Nov 11 — the only consecutive pair is Jun 22 → Jun 23.
        assert_eq!(recap.longest_streak, 2);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn year_drops_events_outside_the_local_year() {
        let (pool, path) = migrated_pool("recap_service_bounds.db").await;
        let service = RecapService::new(pool.clone());

        seed_media(&pool, "m-1", "anime", "Series").await;
        seed_activity(&pool, "a-1", "m-1", "added", &at_local(2025, 12, 31, 23, 0)).await;
        seed_activity(
            &pool,
            "a-2",
            "m-1",
            "completed",
            &at_local(2027, 1, 1, 1, 0),
        )
        .await;

        let recap = service.year(2026).await.expect("recap");
        assert_eq!(recap.totals.added, 0);
        assert_eq!(recap.totals.completed, 0);
        assert_eq!(recap.best_month, None);
        assert_eq!(recap.longest_streak, 0);
        assert!(recap.top_media.is_empty());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn rejects_invalid_years() {
        let (pool, path) = migrated_pool("recap_service_validate.db").await;
        let service = RecapService::new(pool.clone());

        assert!(service.year(1899).await.is_err());
        assert!(service.year(2101).await.is_err());
        pool.close().await;
        cleanup_files(&path);
    }
}
