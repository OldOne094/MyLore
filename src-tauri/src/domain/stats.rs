//! Stats service (MISSION-027).
//!
//! Pure dashboard computations over the tracking/review/progress data: counts,
//! hours, completion, average rating and distributions. Side-effect free — the
//! caller supplies one lightweight row per tracked media.
//!
//! Time is reported from real data only: `consumed_minutes` sums node-level
//! completed minutes (episodes carrying a duration). Reading progress is
//! reported as pages (`consumed_pages`); converting pages to time is a product
//! decision and deliberately not invented here.

use std::collections::BTreeMap;

use crate::domain::enums::{ContentType, CoreStatus};
use crate::domain::progress::{ProgressAggregate, UnitWeight};
use crate::domain::value_objects::{MediaId, Rating};

/// One tracked media's contribution to the stats (a projection of tracking +
/// review + progress so the service stays free of I/O).
#[derive(Debug, Clone)]
pub struct MediaStatsRow {
    pub media_id: MediaId,
    pub content_type: ContentType,
    pub core_status: CoreStatus,
    pub rating: Option<Rating>,
    pub favorite: bool,
    pub release_year: Option<u16>,
    /// The derived progress aggregate for this media.
    pub progress: ProgressAggregate,
}

/// Aggregated dashboard statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct StatsSummary {
    /// Number of tracked media.
    pub total: u64,
    /// Count per core status, in schema order.
    pub status_counts: Vec<(CoreStatus, u64)>,
    /// Count per content type, in schema order.
    pub content_type_counts: Vec<(ContentType, u64)>,
    /// Count per rating value 1..=10.
    pub rating_counts: Vec<(u8, u64)>,
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
    /// Sum of completed pages (book-style aggregates).
    pub consumed_pages: u64,
    /// Count per release year (ascending).
    pub year_counts: Vec<(u16, u64)>,
}

impl StatsSummary {
    /// `consumed_minutes` expressed in hours.
    pub fn consumed_hours(&self) -> f64 {
        self.consumed_minutes as f64 / 60.0
    }
}

/// Compute the full dashboard summary for a set of tracked media.
pub fn compute_stats(rows: &[MediaStatsRow]) -> StatsSummary {
    let mut status_counts: Vec<(CoreStatus, u64)> =
        CoreStatus::ALL.iter().map(|s| (*s, 0)).collect();
    let mut content_type_counts: Vec<(ContentType, u64)> =
        ContentType::ALL.iter().map(|t| (*t, 0)).collect();
    let mut rating_counts: Vec<(u8, u64)> = (Rating::MIN..=Rating::MAX).map(|r| (r, 0)).collect();

    let mut rating_sum: u64 = 0;
    let mut rating_count: u64 = 0;
    let mut favorites: u64 = 0;
    let mut completed: u64 = 0;
    let mut percent_sum: u64 = 0;
    let mut percent_count: u64 = 0;
    let mut consumed_minutes: u64 = 0;
    let mut consumed_pages: u64 = 0;
    let mut years: BTreeMap<u16, u64> = BTreeMap::new();

    for row in rows {
        bump(&mut status_counts, row.core_status);
        bump(&mut content_type_counts, row.content_type);

        if let Some(rating) = row.rating {
            let entry = rating_counts
                .iter_mut()
                .find(|(value, _)| *value == rating.get())
                .expect("rating within 1..=10");
            entry.1 += 1;
            rating_sum += u64::from(rating.get());
            rating_count += 1;
        }

        if row.favorite {
            favorites += 1;
        }
        if row.core_status == CoreStatus::Completed {
            completed += 1;
        }

        if let Some(percent) = row.progress.percent {
            percent_sum += u64::from(percent);
            percent_count += 1;
        }

        if let Some(minutes) = row.progress.completed_minutes {
            consumed_minutes += minutes;
        }
        if row.progress.template.weight == UnitWeight::Pages {
            consumed_pages += row.progress.completed_units;
        }

        if let Some(year) = row.release_year {
            *years.entry(year).or_insert(0) += 1;
        }
    }

    StatsSummary {
        total: rows.len() as u64,
        status_counts,
        content_type_counts,
        rating_counts,
        avg_rating: mean(rating_sum, rating_count),
        favorites,
        completed_media: completed,
        completion_rate: mean(completed, rows.len() as u64),
        avg_percent: mean(percent_sum, percent_count),
        consumed_minutes,
        consumed_pages,
        year_counts: years.into_iter().collect(),
    }
}

fn bump<T: Copy + Eq>(counts: &mut [(T, u64)], value: T) {
    if let Some(entry) = counts.iter_mut().find(|(v, _)| *v == value) {
        entry.1 += 1;
    }
}

fn mean(sum: u64, count: u64) -> Option<f64> {
    (count > 0).then(|| sum as f64 / count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::enums::{NodeKind, NodeProgressState};
    use crate::domain::progress::{aggregate, NodeTick};
    use crate::domain::value_objects::{MediaId, Rating};

    fn row(
        id: &str,
        content_type: ContentType,
        status: CoreStatus,
        rating: Option<u8>,
        favorite: bool,
        year: Option<u16>,
        progress: ProgressAggregate,
    ) -> MediaStatsRow {
        MediaStatsRow {
            media_id: MediaId::new(id).unwrap(),
            content_type,
            core_status: status,
            rating: rating.map(|r| Rating::new(r.into()).unwrap()),
            favorite,
            release_year: year,
            progress,
        }
    }

    fn episode(id: &str, state: NodeProgressState) -> NodeTick {
        NodeTick {
            id: id.into(),
            kind: NodeKind::Episode,
            state,
            page_count: None,
            duration_min: Some(24),
        }
    }

    fn anime_progress(completed: u32, total: u32) -> ProgressAggregate {
        let mut nodes = (0..completed)
            .map(|i| episode(&format!("e{i}"), NodeProgressState::Watched))
            .collect::<Vec<_>>();
        nodes.extend(
            (completed..total).map(|i| episode(&format!("e{i}"), NodeProgressState::Unread)),
        );
        aggregate(ContentType::Anime, &nodes)
    }

    fn chapter_progress(completed_pages: u32, total_pages: u32) -> ProgressAggregate {
        let nodes = [
            NodeTick {
                id: "c1".into(),
                kind: NodeKind::Chapter,
                state: NodeProgressState::Read,
                page_count: Some(completed_pages),
                duration_min: None,
            },
            NodeTick {
                id: "c2".into(),
                kind: NodeKind::Chapter,
                state: NodeProgressState::Unread,
                page_count: Some(total_pages - completed_pages),
                duration_min: None,
            },
        ];
        aggregate(ContentType::Book, &nodes)
    }

    #[test]
    fn counts_statuses_and_content_types() {
        let rows = [
            row(
                "m-1",
                ContentType::Anime,
                CoreStatus::InProgress,
                None,
                false,
                None,
                anime_progress(3, 12),
            ),
            row(
                "m-2",
                ContentType::Anime,
                CoreStatus::Completed,
                None,
                false,
                None,
                anime_progress(12, 12),
            ),
            row(
                "m-3",
                ContentType::Book,
                CoreStatus::Planned,
                None,
                false,
                None,
                aggregate(ContentType::Book, &[]),
            ),
        ];
        let stats = compute_stats(&rows);

        assert_eq!(stats.total, 3);
        let in_progress = stats
            .status_counts
            .iter()
            .find(|(s, _)| *s == CoreStatus::InProgress)
            .unwrap()
            .1;
        let completed = stats
            .status_counts
            .iter()
            .find(|(s, _)| *s == CoreStatus::Completed)
            .unwrap()
            .1;
        let planned = stats
            .status_counts
            .iter()
            .find(|(s, _)| *s == CoreStatus::Planned)
            .unwrap()
            .1;
        assert_eq!((in_progress, completed, planned), (1, 1, 1));

        let anime = stats
            .content_type_counts
            .iter()
            .find(|(t, _)| *t == ContentType::Anime)
            .unwrap()
            .1;
        let book = stats
            .content_type_counts
            .iter()
            .find(|(t, _)| *t == ContentType::Book)
            .unwrap()
            .1;
        assert_eq!((anime, book), (2, 1));
    }

    #[test]
    fn average_rating_and_rating_distribution() {
        let rows = [
            row(
                "m-1",
                ContentType::Anime,
                CoreStatus::Completed,
                Some(9),
                true,
                None,
                anime_progress(12, 12),
            ),
            row(
                "m-2",
                ContentType::Anime,
                CoreStatus::Completed,
                Some(7),
                false,
                None,
                anime_progress(12, 12),
            ),
            row(
                "m-3",
                ContentType::Book,
                CoreStatus::Planned,
                None,
                false,
                None,
                aggregate(ContentType::Book, &[]),
            ),
        ];
        let stats = compute_stats(&rows);

        assert_eq!(stats.avg_rating, Some(8.0));
        assert_eq!(stats.favorites, 1);
        let nine = stats.rating_counts.iter().find(|(r, _)| *r == 9).unwrap().1;
        let seven = stats.rating_counts.iter().find(|(r, _)| *r == 7).unwrap().1;
        assert_eq!((seven, nine), (1, 1));
        assert!(stats
            .rating_counts
            .iter()
            .all(|(r, _)| (1..=10).contains(r)));
    }

    #[test]
    fn completion_and_average_percent() {
        let rows = [
            row(
                "m-1",
                ContentType::Anime,
                CoreStatus::Completed,
                None,
                false,
                None,
                anime_progress(12, 12),
            ),
            row(
                "m-2",
                ContentType::Anime,
                CoreStatus::Completed,
                None,
                false,
                None,
                anime_progress(6, 12),
            ),
            row(
                "m-3",
                ContentType::Book,
                CoreStatus::Planned,
                None,
                false,
                None,
                aggregate(ContentType::Book, &[]),
            ),
        ];
        let stats = compute_stats(&rows);

        assert_eq!(stats.completed_media, 2);
        assert_eq!(stats.completion_rate, Some(2.0 / 3.0));
        // (100 + 50) / 2; the planned book has no percent.
        assert_eq!(stats.avg_percent, Some(75.0));
    }

    #[test]
    fn consumed_minutes_and_pages() {
        let rows = [
            row(
                "m-1",
                ContentType::Anime,
                CoreStatus::InProgress,
                None,
                false,
                None,
                anime_progress(5, 12),
            ),
            row(
                "m-2",
                ContentType::Book,
                CoreStatus::InProgress,
                None,
                false,
                None,
                chapter_progress(120, 400),
            ),
        ];
        let stats = compute_stats(&rows);

        // 5 watched episodes × 24 min.
        assert_eq!(stats.consumed_minutes, 120);
        assert_eq!(stats.consumed_hours(), 2.0);
        // Book template weighs by pages: completed_units == pages read.
        assert_eq!(stats.consumed_pages, 120);
    }

    #[test]
    fn empty_library_is_zero_and_empty() {
        let stats = compute_stats(&[]);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.completion_rate, None);
        assert_eq!(stats.avg_rating, None);
        assert_eq!(stats.avg_percent, None);
        assert_eq!(stats.consumed_hours(), 0.0);
        assert!(stats.status_counts.iter().all(|(_, c)| *c == 0));
        assert!(stats.rating_counts.iter().all(|(_, c)| *c == 0));
        assert!(stats.year_counts.is_empty());
    }

    #[test]
    fn year_distribution_is_ascending() {
        let rows = [
            row(
                "m-1",
                ContentType::Anime,
                CoreStatus::Completed,
                None,
                false,
                Some(2011),
                anime_progress(12, 12),
            ),
            row(
                "m-2",
                ContentType::Anime,
                CoreStatus::Completed,
                None,
                false,
                Some(2024),
                anime_progress(12, 12),
            ),
            row(
                "m-3",
                ContentType::Anime,
                CoreStatus::Completed,
                None,
                false,
                Some(2011),
                anime_progress(12, 12),
            ),
        ];
        let stats = compute_stats(&rows);
        assert_eq!(stats.year_counts, vec![(2011, 2), (2024, 1)]);
    }

    #[test]
    fn planned_with_no_nodes_contributes_nothing_to_time() {
        let rows = [row(
            "m-1",
            ContentType::Anime,
            CoreStatus::Planned,
            None,
            false,
            None,
            aggregate(ContentType::Anime, &[]),
        )];
        let stats = compute_stats(&rows);
        assert_eq!(stats.consumed_minutes, 0);
        assert_eq!(stats.avg_percent, None);
    }
}
