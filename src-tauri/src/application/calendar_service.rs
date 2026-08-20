//! Calendar service (MISSION-081).
//!
//! Resolves one month of the local calendar: content-node air/release dates
//! (per-unit release_date) plus the user activity trail, bucketed per local
//! day. Activity timestamps are RFC3339 UTC — the service converts each event
//! to the user's local timezone before bucketing, and queries a window wide
//! enough (±1 day) that any event whose *local* date falls inside the month is
//! captured regardless of the machine's UTC offset.

use chrono::{DateTime, Datelike, Days, Local, NaiveDate};
use serde::Serialize;
use sqlx::sqlite::SqlitePool;

use crate::application::progress_service::unit_label;
use crate::error::AppError;
use crate::infrastructure::repositories::calendar::{activity_in_range, air_dates};

/// One calendar entry: an air/release date or an activity event.
#[derive(Debug, Clone, Serialize)]
pub struct CalendarItem {
    pub media_id: Option<String>,
    pub title: String,
    pub content_type: Option<String>,
    /// Air events carry the unit label ("E5", "Ch3"); activity events do not.
    pub label: Option<String>,
    /// Activity events carry their kind ("started", "completed", …); airs do not.
    pub kind: Option<String>,
    /// Activity events carry the local wall-clock time ("HH:MM"); airs do not.
    pub time: Option<String>,
}

/// All events for one calendar day (empty arrays when the day is quiet).
#[derive(Debug, Clone, Serialize)]
pub struct CalendarDay {
    pub date: String,
    pub airs: Vec<CalendarItem>,
    pub activity: Vec<CalendarItem>,
}

/// A full calendar month.
#[derive(Debug, Clone, Serialize)]
pub struct CalendarMonth {
    pub year: u16,
    pub month: u8,
    pub days: Vec<CalendarDay>,
}

/// Backend for the calendar page.
pub struct CalendarService {
    pool: SqlitePool,
}

impl CalendarService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Resolve one month of air dates + activity.
    pub async fn month(&self, year: u16, month: u8) -> Result<CalendarMonth, AppError> {
        if !(1900..=2100).contains(&year) {
            return Err(AppError::validation(format!("year out of range: {year}")));
        }
        if !(1..=12).contains(&month) {
            return Err(AppError::validation(format!("month out of range: {month}")));
        }

        let start = NaiveDate::from_ymd_opt(year as i32, month as u32, 1).expect("validated month");
        let next = next_month_start(year, month);
        let last_day = (next - Days::new(1)).day();

        let mut days: Vec<CalendarDay> = (1..=last_day)
            .map(|d| CalendarDay {
                date: format!("{year:04}-{month:02}-{d:02}"),
                airs: vec![],
                activity: vec![],
            })
            .collect();

        // Air dates are bare ISO dates — query the month window directly.
        let airs = air_dates(
            &self.pool,
            &start.format("%Y-%m-%d").to_string(),
            &next.format("%Y-%m-%d").to_string(),
        )
        .await?;
        for row in airs {
            let Ok(date) = NaiveDate::parse_from_str(&row.release_date, "%Y-%m-%d") else {
                continue;
            };
            let index = date.day() as usize;
            if let Some(day) = days.get_mut(index - 1) {
                day.airs.push(CalendarItem {
                    media_id: Some(row.media_id),
                    title: row.title,
                    content_type: Some(row.content_type),
                    label: Some(unit_label(
                        &row.node_kind,
                        row.node_number.as_deref(),
                        row.node_position,
                    )),
                    kind: None,
                    time: None,
                });
            }
        }

        // Activity is RFC3339 UTC; query a window wide enough for any local
        // offset (±1 day) and bucket by each event's local date.
        let lo = (start - Days::new(1)).format("%Y-%m-%d").to_string();
        let hi = (next + Days::new(1)).format("%Y-%m-%d").to_string();
        let events = activity_in_range(&self.pool, &lo, &hi).await?;
        for row in events {
            let Ok(dt) = DateTime::parse_from_rfc3339(&row.created_at) else {
                continue;
            };
            let local = dt.with_timezone(&Local);
            if local.year() != year as i32 || local.month() != month as u32 {
                continue;
            }
            let index = local.day() as usize;
            if let Some(day) = days.get_mut(index - 1) {
                day.activity.push(CalendarItem {
                    media_id: row.media_id,
                    title: row.title.unwrap_or_default(),
                    content_type: row.content_type,
                    label: None,
                    kind: Some(row.kind),
                    time: Some(local.format("%H:%M").to_string()),
                });
            }
        }

        Ok(CalendarMonth { year, month, days })
    }
}

fn next_month_start(year: u16, month: u8) -> NaiveDate {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(y as i32, m as u32, 1).expect("valid next month")
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

    async fn seed_node(
        pool: &SqlitePool,
        id: &str,
        media_id: &str,
        kind: &str,
        number: &str,
        release_date: &str,
    ) {
        sqlx::query(
            "INSERT INTO content_node (id, media_id, kind, position, number, release_date, created_at)
             VALUES (?, ?, ?, 1, ?, ?, '2026-01-01')",
        )
        .bind(id)
        .bind(media_id)
        .bind(kind)
        .bind(number)
        .bind(release_date)
        .execute(pool)
        .await
        .expect("seed node");
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
    async fn month_buckets_air_dates_and_activity_by_day() {
        let (pool, path) = migrated_pool("calendar_service.db").await;
        let service = CalendarService::new(pool.clone());

        seed_media(&pool, "m-1", "anime", "Series").await;
        seed_media(&pool, "m-2", "novel", "Book").await;
        seed_node(&pool, "n-1", "m-1", "episode", "5", "2026-08-05").await;
        seed_node(&pool, "n-2", "m-1", "episode", "6", "2026-08-05").await;
        seed_node(&pool, "n-3", "m-1", "episode", "7", "2026-08-12").await;

        seed_activity(&pool, "a-1", "m-1", "started", &at_local(2026, 8, 5, 9, 0)).await;
        seed_activity(
            &pool,
            "a-2",
            "m-2",
            "progress",
            &at_local(2026, 8, 5, 21, 30),
        )
        .await;
        seed_activity(
            &pool,
            "a-3",
            "m-1",
            "completed",
            &at_local(2026, 8, 6, 18, 0),
        )
        .await;

        let month = service.month(2026, 8).await.expect("month");

        assert_eq!(month.year, 2026);
        assert_eq!(month.month, 8);
        assert_eq!(month.days.len(), 31);
        assert_eq!(month.days[0].date, "2026-08-01");

        let day5 = &month.days[4];
        assert_eq!(day5.airs.len(), 2, "two episodes air on the 5th");
        assert_eq!(day5.airs[0].label.as_deref(), Some("E5"));
        assert_eq!(day5.airs[0].title, "Series");
        assert_eq!(day5.airs[0].content_type.as_deref(), Some("anime"));
        assert_eq!(day5.activity.len(), 2);
        assert_eq!(day5.activity[0].kind.as_deref(), Some("started"));
        assert_eq!(day5.activity[0].time.as_deref(), Some("09:00"));
        assert_eq!(day5.activity[1].kind.as_deref(), Some("progress"));
        assert_eq!(day5.activity[1].time.as_deref(), Some("21:30"));

        let day12 = &month.days[11];
        assert_eq!(day12.airs.len(), 1);
        assert_eq!(day12.airs[0].label.as_deref(), Some("E7"));
        assert!(day12.activity.is_empty());

        let day6 = &month.days[5];
        assert_eq!(day6.activity.len(), 1, "the completed event is its own day");
        assert_eq!(day6.activity[0].kind.as_deref(), Some("completed"));
        assert_eq!(day6.activity[0].time.as_deref(), Some("18:00"));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn activity_outside_the_month_is_dropped() {
        let (pool, path) = migrated_pool("calendar_service_bounds.db").await;
        let service = CalendarService::new(pool.clone());

        seed_media(&pool, "m-1", "anime", "Series").await;
        seed_activity(
            &pool,
            "a-1",
            "m-1",
            "started",
            &at_local(2026, 7, 31, 23, 30),
        )
        .await;
        seed_activity(
            &pool,
            "a-2",
            "m-1",
            "completed",
            &at_local(2026, 9, 1, 0, 15),
        )
        .await;

        let month = service.month(2026, 8).await.expect("month");
        assert!(
            month.days.iter().all(|d| d.activity.is_empty()),
            "no activity lands in August"
        );
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn february_has_28_days_all_empty() {
        let (pool, path) = migrated_pool("calendar_service_feb.db").await;
        let service = CalendarService::new(pool.clone());

        let month = service.month(2026, 2).await.expect("month");
        assert_eq!(month.days.len(), 28);
        assert!(month
            .days
            .iter()
            .all(|d| d.airs.is_empty() && d.activity.is_empty()));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn rejects_invalid_year_and_month() {
        let (pool, path) = migrated_pool("calendar_service_validate.db").await;
        let service = CalendarService::new(pool.clone());

        assert!(service.month(1899, 1).await.is_err());
        assert!(service.month(2026, 0).await.is_err());
        assert!(service.month(2026, 13).await.is_err());
        assert!(service.month(2101, 1).await.is_err());
        pool.close().await;
        cleanup_files(&path);
    }
}
