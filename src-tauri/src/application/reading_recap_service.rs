//! Reading recap service (MISSION-083).
//!
//! StoryGraph-style reading stats from local data only: pages & chapters
//! consumed per month of a chosen year plus all-time taste distributions —
//! mood set, pace and format — built from the review metadata (MISSION-079)
//! and the tracked reading media. Pages are weighed exactly like the progress
//! engine: a book chapter counts its `page_count` (or 1), non-book reading
//! chapters count 0 pages. Like the calendar/recap services, consumption is
//! bucketed by the user's *local* time with a ±1-day query window.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate};
use serde::Serialize;
use sqlx::sqlite::SqlitePool;

use crate::application::stats_service::StatCount;
use crate::error::AppError;
use crate::infrastructure::repositories::calendar::activity_in_range;
use crate::infrastructure::repositories::reading::{monthly_reading, reading_formats, taste_rows};

fn is_reading(content_type: &str) -> bool {
    matches!(
        content_type,
        "book" | "novel" | "web_novel" | "manga" | "manhwa" | "manhua"
    )
}

/// Consumption for one month.
#[derive(Debug, Clone, Serialize)]
pub struct MonthReading {
    /// Book pages consumed (book chapters weighed by page count, 1 when unknown).
    pub pages: u32,
    /// Chapters consumed across all reading media.
    pub chapters: u32,
}

/// Headline numbers for the year.
#[derive(Debug, Clone, Serialize)]
pub struct ReadingTotals {
    pub pages: u32,
    pub chapters: u32,
    /// Distinct reading media completed in the year.
    pub finished: u32,
}

/// The serializable reading recap.
#[derive(Debug, Clone, Serialize)]
pub struct ReadingRecap {
    pub year: u16,
    /// Pages/chapters per month (12 entries, index = month − 1).
    pub by_month: Vec<MonthReading>,
    pub totals: ReadingTotals,
    /// Mood keys ranked by how many reviews carry them (all-time).
    pub mood_counts: Vec<StatCount>,
    /// Pace values ranked by review count (all-time; no pace = absent).
    pub pace_counts: Vec<StatCount>,
    /// Formats of tracked reading media ranked by media count (all-time).
    pub format_counts: Vec<StatCount>,
}

/// Backend for the reading recap.
pub struct ReadingRecapService {
    pool: SqlitePool,
}

impl ReadingRecapService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Resolve the reading recap for one year.
    pub async fn recap(&self, year: u16) -> Result<ReadingRecap, AppError> {
        if !(1900..=2100).contains(&year) {
            return Err(AppError::validation(format!("year out of range: {year}")));
        }

        let start = NaiveDate::from_ymd_opt(year as i32, 1, 1).expect("valid year");
        let next = NaiveDate::from_ymd_opt(year as i32 + 1, 1, 1).expect("valid next year");
        // Window wide enough for any local offset (±1 day); filter by each
        // event's local year below.
        let lo = (start - Days::new(1)).format("%Y-%m-%d").to_string();
        let hi = (next + Days::new(1)).format("%Y-%m-%d").to_string();

        let mut by_month = vec![
            MonthReading {
                pages: 0,
                chapters: 0
            };
            12
        ];
        let rows = monthly_reading(&self.pool, &lo, &hi).await?;
        for row in rows {
            let Ok(dt) = DateTime::parse_from_rfc3339(&row.read_at) else {
                continue;
            };
            let local = dt.with_timezone(&Local);
            if local.year() != year as i32 {
                continue;
            }
            let bucket = &mut by_month[local.month() as usize - 1];
            bucket.chapters += 1;
            if row.content_type == "book" {
                bucket.pages += row.page_count.unwrap_or(1).max(0) as u32;
            }
        }

        let mut finished = HashSet::new();
        let events = activity_in_range(&self.pool, &lo, &hi).await?;
        for row in events {
            let Some(content_type) = row.content_type.as_deref() else {
                continue;
            };
            if row.kind != "completed" || !is_reading(content_type) {
                continue;
            }
            let Ok(dt) = DateTime::parse_from_rfc3339(&row.created_at) else {
                continue;
            };
            if dt.with_timezone(&Local).year() != year as i32 {
                continue;
            }
            if let Some(id) = row.media_id {
                finished.insert(id);
            }
        }

        let pages: u32 = by_month.iter().map(|m| m.pages).sum();
        let chapters: u32 = by_month.iter().map(|m| m.chapters).sum();

        let (mood_counts, pace_counts) = taste_counts(&self.pool).await?;
        let format_counts = reading_formats(&self.pool)
            .await?
            .into_iter()
            .map(|row| StatCount {
                key: row.format,
                count: u64::from(row.count),
            })
            .collect();

        Ok(ReadingRecap {
            year,
            by_month,
            totals: ReadingTotals {
                pages,
                chapters,
                finished: finished.len() as u32,
            },
            mood_counts,
            pace_counts,
            format_counts,
        })
    }
}

/// Fold every review's moods + pace into ranked distributions.
async fn taste_counts(pool: &SqlitePool) -> Result<(Vec<StatCount>, Vec<StatCount>), AppError> {
    let mut moods: HashMap<String, u64> = HashMap::new();
    let mut paces: HashMap<String, u64> = HashMap::new();
    for row in taste_rows(pool).await? {
        if let Some(raw) = row.moods {
            if let Ok(keys) = serde_json::from_str::<Vec<String>>(&raw) {
                for key in keys {
                    *moods.entry(key).or_default() += 1;
                }
            }
        }
        if let Some(pace) = row.pace {
            *paces.entry(pace).or_default() += 1;
        }
    }
    Ok((rank(&moods), rank(&paces)))
}

/// Sort a distribution by count desc, then key asc.
fn rank(counts: &HashMap<String, u64>) -> Vec<StatCount> {
    let mut out: Vec<StatCount> = counts
        .iter()
        .map(|(key, count)| StatCount {
            key: key.clone(),
            count: *count,
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};
    use chrono::{TimeZone, Utc};

    async fn seed_media(pool: &SqlitePool, id: &str, content_type: &str, format: Option<&str>) {
        sqlx::query(
            "INSERT INTO media (id, content_type, format, title_main, created_at, updated_at)
             VALUES (?, ?, ?, 'Title', '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .bind(content_type)
        .bind(format)
        .execute(pool)
        .await
        .expect("seed media");
    }

    async fn seed_chapter(pool: &SqlitePool, id: &str, media_id: &str, page_count: Option<i64>) {
        sqlx::query(
            "INSERT INTO content_node (id, media_id, kind, position, number, page_count, created_at)
             VALUES (?, ?, 'chapter', 1, ?, ?, '2026-01-01')",
        )
        .bind(id)
        .bind(media_id)
        .bind(id)
        .bind(page_count)
        .execute(pool)
        .await
        .expect("seed chapter");
    }

    async fn read(pool: &SqlitePool, node_id: &str, at: &str) {
        sqlx::query(
            "INSERT INTO node_progress (node_id, state, read_at, updated_at)
             VALUES (?, 'read', ?, ?)",
        )
        .bind(node_id)
        .bind(at)
        .bind(at)
        .execute(pool)
        .await
        .expect("mark read");
    }

    async fn track(pool: &SqlitePool, media_id: &str) {
        sqlx::query(
            "INSERT INTO tracking (media_id, core_status, auto_track, updated_at)
             VALUES (?, 'in_progress', 1, '2026-01-01')",
        )
        .bind(media_id)
        .execute(pool)
        .await
        .expect("track");
    }

    async fn review(pool: &SqlitePool, media_id: &str, moods: &[&str], pace: Option<&str>) {
        let moods = serde_json::to_string(moods).expect("moods json");
        sqlx::query(
            "INSERT INTO review (media_id, moods, pace, created_at, updated_at)
             VALUES (?, ?, ?, '2026-01-02', '2026-01-02')",
        )
        .bind(media_id)
        .bind(moods)
        .bind(pace)
        .execute(pool)
        .await
        .expect("seed review");
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
    async fn recap_buckets_pages_and_chapters_and_taste() {
        let (pool, path) = migrated_pool("reading_recap.db").await;
        let service = ReadingRecapService::new(pool.clone());

        seed_media(&pool, "m-1", "book", Some("light_novel")).await;
        seed_media(&pool, "m-2", "manga", Some("webtoon")).await;
        seed_media(&pool, "m-3", "novel", None).await;
        seed_chapter(&pool, "c1", "m-1", Some(120)).await;
        seed_chapter(&pool, "c2", "m-1", Some(80)).await;
        seed_chapter(&pool, "c3", "m-2", None).await;
        seed_chapter(&pool, "c4", "m-3", None).await;
        read(&pool, "c1", &at_local(2026, 3, 10, 9, 0)).await;
        read(&pool, "c2", &at_local(2026, 6, 5, 18, 0)).await;
        read(&pool, "c3", &at_local(2026, 6, 20, 19, 0)).await;
        read(&pool, "c4", &at_local(2026, 6, 22, 20, 0)).await;
        track(&pool, "m-1").await;
        track(&pool, "m-2").await;
        review(&pool, "m-1", &["dark", "tense"], Some("medium")).await;
        review(&pool, "m-2", &["dark"], Some("fast")).await;

        sqlx::query(
            "INSERT INTO activity (id, media_id, kind, created_at)
             VALUES ('a-1', 'm-1', 'completed', ?)",
        )
        .bind(at_local(2026, 6, 22, 21, 0))
        .execute(&pool)
        .await
        .expect("seed activity");

        let recap = service.recap(2026).await.expect("recap");

        assert_eq!(recap.by_month.len(), 12);
        assert_eq!(recap.by_month[2].pages, 120);
        assert_eq!(recap.by_month[2].chapters, 1);
        assert_eq!(
            recap.by_month[5].pages, 80,
            "manga/novel chapters add no pages"
        );
        assert_eq!(recap.by_month[5].chapters, 3);

        assert_eq!(recap.totals.pages, 200);
        assert_eq!(recap.totals.chapters, 4);
        assert_eq!(
            recap.totals.finished, 1,
            "m-1 is the completed reading media"
        );

        assert_eq!(
            recap.mood_counts,
            vec![
                StatCount {
                    key: "dark".into(),
                    count: 2
                },
                StatCount {
                    key: "tense".into(),
                    count: 1
                },
            ]
        );
        assert_eq!(
            recap.pace_counts,
            vec![
                StatCount {
                    key: "fast".into(),
                    count: 1
                },
                StatCount {
                    key: "medium".into(),
                    count: 1
                },
            ]
        );
        assert_eq!(
            recap.format_counts,
            vec![
                StatCount {
                    key: "light_novel".into(),
                    count: 1
                },
                StatCount {
                    key: "webtoon".into(),
                    count: 1
                },
            ]
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn recap_drops_consumption_outside_the_local_year() {
        let (pool, path) = migrated_pool("reading_recap_bounds.db").await;
        let service = ReadingRecapService::new(pool.clone());

        seed_media(&pool, "m-1", "book", None).await;
        seed_chapter(&pool, "c1", "m-1", Some(50)).await;
        seed_chapter(&pool, "c2", "m-1", Some(50)).await;
        read(&pool, "c1", &at_local(2025, 12, 31, 23, 0)).await;
        read(&pool, "c2", &at_local(2027, 1, 1, 1, 0)).await;

        let recap = service.recap(2026).await.expect("recap");
        assert!(recap
            .by_month
            .iter()
            .all(|m| m.chapters == 0 && m.pages == 0));
        assert_eq!(recap.totals.finished, 0);
        assert!(recap.format_counts.is_empty());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn rejects_invalid_years() {
        let (pool, path) = migrated_pool("reading_recap_validate.db").await;
        let service = ReadingRecapService::new(pool.clone());

        assert!(service.recap(1899).await.is_err());
        assert!(service.recap(2101).await.is_err());
        pool.close().await;
        cleanup_files(&path);
    }
}
