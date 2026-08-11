//! Progress engine (MISSION-023, DOMAIN_MODEL §2.3).
//!
//! Aggregate progress is *derived* from per-node states — it is never stored
//! (REQ-TRACK-004). Each content type has a **progress template** that defines:
//!   - which node kind is the countable unit (episode / chapter / …),
//!   - how one unit weighs into the total (1, pages, minutes),
//!   - which node state counts as "consumed" (read / watched).
//!
//! `aggregate` folds a set of node ticks into `ProgressAggregate`; when the
//! node tree is incomplete (e.g. an ongoing web novel), `estimated_total_units`
//! + `with_estimate` fall back to the media-level runtime counters.

use crate::domain::enums::{ContentType, NodeKind, NodeProgressState};
use crate::domain::media::MediaRuntime;

/// How one countable node weighs into the aggregate total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitWeight {
    /// Each node is exactly one unit (episodes, chapters, volumes).
    Count,
    /// A node contributes its `page_count` (books); a chapter without a page
    /// count contributes 1.
    Pages,
}

/// The progress template for one content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressTemplate {
    pub content_type: ContentType,
    /// The node kind that counts as a progress unit.
    pub unit_kind: NodeKind,
    pub weight: UnitWeight,
    /// The state that marks a unit consumed.
    pub consuming_state: NodeProgressState,
}

impl ProgressTemplate {
    /// The template for a content type (all content types are covered).
    pub fn for_content_type(content_type: ContentType) -> ProgressTemplate {
        use ContentType::*;
        match content_type {
            Anime | Tv | Movie => ProgressTemplate {
                content_type,
                unit_kind: NodeKind::Episode,
                weight: UnitWeight::Count,
                consuming_state: NodeProgressState::Watched,
            },
            Manga | Manhwa | Manhua | Novel | WebNovel => ProgressTemplate {
                content_type,
                unit_kind: NodeKind::Chapter,
                weight: UnitWeight::Count,
                consuming_state: NodeProgressState::Read,
            },
            Book => ProgressTemplate {
                content_type,
                unit_kind: NodeKind::Chapter,
                weight: UnitWeight::Pages,
                consuming_state: NodeProgressState::Read,
            },
            Other => ProgressTemplate {
                content_type,
                unit_kind: NodeKind::Node,
                weight: UnitWeight::Count,
                consuming_state: NodeProgressState::Read,
            },
        }
    }

    /// A UI-ready label for the progress unit (plural).
    pub fn unit_label(&self) -> &'static str {
        match self.weight {
            UnitWeight::Pages => "pages",
            UnitWeight::Count => match self.unit_kind {
                NodeKind::Episode => "episodes",
                NodeKind::Chapter => "chapters",
                NodeKind::Volume => "volumes",
                NodeKind::Season => "seasons",
                _ => "units",
            },
        }
    }
}

/// A node's contribution to the aggregate (a projection of `ContentNode` +
/// its `NodeProgress`, so the engine stays free of I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeTick {
    pub id: String,
    pub kind: NodeKind,
    pub state: NodeProgressState,
    pub page_count: Option<u32>,
    pub duration_min: Option<u32>,
}

/// The derived progress of one media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressAggregate {
    pub template: ProgressTemplate,
    /// Weighted total of countable nodes.
    pub total_units: u64,
    /// Weighted total of consumed nodes.
    pub completed_units: u64,
    /// `completed_units / total_units`, rounded down; `None` when there are no
    /// countable nodes.
    pub percent: Option<u8>,
    /// Total minutes of countable nodes (only when at least one carries a
    /// duration).
    pub total_minutes: Option<u64>,
    /// Minutes of consumed nodes.
    pub completed_minutes: Option<u64>,
}

/// Fold node ticks into a progress aggregate for the content type.
///
/// Only nodes whose kind matches the template's `unit_kind` count. `Partial`
/// does not count as consumed; minutes aggregate from any countable node that
/// carries `duration_min`.
pub fn aggregate(content_type: ContentType, nodes: &[NodeTick]) -> ProgressAggregate {
    let template = ProgressTemplate::for_content_type(content_type);
    let mut aggregate = ProgressAggregate {
        template,
        total_units: 0,
        completed_units: 0,
        percent: None,
        total_minutes: None,
        completed_minutes: None,
    };

    for tick in nodes {
        if tick.kind != aggregate.template.unit_kind {
            continue;
        }
        let weight = match aggregate.template.weight {
            UnitWeight::Count => 1,
            UnitWeight::Pages => u64::from(tick.page_count.unwrap_or(1)),
        };
        aggregate.total_units += weight;
        if tick.state == aggregate.template.consuming_state {
            aggregate.completed_units += weight;
        }
        if let Some(minutes) = tick.duration_min {
            let minutes = u64::from(minutes);
            *aggregate.total_minutes.get_or_insert(0) += minutes;
            if tick.state == aggregate.template.consuming_state {
                *aggregate.completed_minutes.get_or_insert(0) += minutes;
            }
        }
    }

    aggregate.percent = (aggregate.total_units > 0)
        .then(|| (aggregate.completed_units.saturating_mul(100) / aggregate.total_units) as u8);
    aggregate
}

impl ProgressAggregate {
    /// Fill the total from a media-level runtime estimate when the node tree
    /// contributed no units (e.g. an ongoing series with no nodes imported).
    /// Percent becomes 0 when an estimate supplies the total.
    pub fn with_estimate(mut self, estimated_total: Option<u64>) -> Self {
        if self.total_units == 0 {
            if let Some(estimated) = estimated_total.filter(|&e| e > 0) {
                self.total_units = estimated;
                self.percent = Some(0);
            }
        }
        self
    }
}

/// A media-level estimate of the total progress units (for "of ~120 chapters"
/// displays and stats when the node tree is incomplete).
pub fn estimated_total_units(content_type: ContentType, runtime: &MediaRuntime) -> Option<u64> {
    let template = ProgressTemplate::for_content_type(content_type);
    match template.weight {
        UnitWeight::Pages => runtime.pages.map(u64::from),
        UnitWeight::Count => match template.unit_kind {
            NodeKind::Episode => runtime.ep_count.map(u64::from),
            NodeKind::Chapter => runtime.ch_count.map(u64::from),
            NodeKind::Volume => runtime.ch_count.map(u64::from),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(
        id: &str,
        kind: NodeKind,
        state: NodeProgressState,
        page_count: Option<u32>,
        duration_min: Option<u32>,
    ) -> NodeTick {
        NodeTick {
            id: id.into(),
            kind,
            state,
            page_count,
            duration_min,
        }
    }

    fn episode(id: &str, state: NodeProgressState) -> NodeTick {
        tick(id, NodeKind::Episode, state, None, Some(24))
    }

    #[test]
    fn every_content_type_has_a_template() {
        for content_type in ContentType::ALL {
            let template = ProgressTemplate::for_content_type(*content_type);
            assert_eq!(template.content_type, *content_type);
        }
    }

    #[test]
    fn anime_counts_watched_episodes() {
        let nodes = [
            episode("e1", NodeProgressState::Watched),
            episode("e2", NodeProgressState::Watched),
            episode("e3", NodeProgressState::Watched),
            episode("e4", NodeProgressState::Unread),
            episode("e5", NodeProgressState::Unread),
        ];
        let agg = aggregate(ContentType::Anime, &nodes);
        assert_eq!(agg.template.unit_kind, NodeKind::Episode);
        assert_eq!(agg.total_units, 5);
        assert_eq!(agg.completed_units, 3);
        assert_eq!(agg.percent, Some(60));
        assert_eq!(agg.total_minutes, Some(120));
        assert_eq!(agg.completed_minutes, Some(72));
    }

    #[test]
    fn manga_counts_read_chapters() {
        let nodes = [
            tick("c1", NodeKind::Chapter, NodeProgressState::Read, None, None),
            tick("c2", NodeKind::Chapter, NodeProgressState::Read, None, None),
            tick(
                "c3",
                NodeKind::Chapter,
                NodeProgressState::Partial,
                None,
                None,
            ),
            tick(
                "c4",
                NodeKind::Chapter,
                NodeProgressState::Unread,
                None,
                None,
            ),
        ];
        let agg = aggregate(ContentType::Manga, &nodes);
        assert_eq!(agg.total_units, 4);
        assert_eq!(agg.completed_units, 2);
        assert_eq!(agg.percent, Some(50));
        assert_eq!(agg.total_minutes, None, "chapters carry no durations");
    }

    #[test]
    fn book_weighs_chapters_by_pages() {
        let nodes = [
            tick(
                "c1",
                NodeKind::Chapter,
                NodeProgressState::Read,
                Some(30),
                None,
            ),
            tick(
                "c2",
                NodeKind::Chapter,
                NodeProgressState::Read,
                Some(25),
                None,
            ),
            tick(
                "c3",
                NodeKind::Chapter,
                NodeProgressState::Unread,
                Some(45),
                None,
            ),
        ];
        let agg = aggregate(ContentType::Book, &nodes);
        assert_eq!(agg.template.weight, UnitWeight::Pages);
        assert_eq!(agg.total_units, 100);
        assert_eq!(agg.completed_units, 55);
        assert_eq!(agg.percent, Some(55));
    }

    #[test]
    fn book_chapter_without_page_count_counts_as_one() {
        let nodes = [
            tick("c1", NodeKind::Chapter, NodeProgressState::Read, None, None),
            tick(
                "c2",
                NodeKind::Chapter,
                NodeProgressState::Unread,
                Some(50),
                None,
            ),
        ];
        let agg = aggregate(ContentType::Book, &nodes);
        assert_eq!(agg.total_units, 51);
        assert_eq!(agg.completed_units, 1);
    }

    #[test]
    fn non_unit_nodes_are_ignored() {
        let nodes = [
            episode("e1", NodeProgressState::Watched),
            tick(
                "s1",
                NodeKind::Season,
                NodeProgressState::Watched,
                None,
                None,
            ),
        ];
        let agg = aggregate(ContentType::Anime, &nodes);
        assert_eq!(agg.total_units, 1, "seasons do not count as episodes");
        assert_eq!(agg.completed_units, 1);
        assert_eq!(agg.percent, Some(100));
    }

    #[test]
    fn empty_tree_has_no_percent() {
        let agg = aggregate(ContentType::Novel, &[]);
        assert_eq!(agg.total_units, 0);
        assert_eq!(agg.percent, None);
    }

    #[test]
    fn movie_is_a_single_episode_unit() {
        let watched = aggregate(
            ContentType::Movie,
            &[episode("m1", NodeProgressState::Watched)],
        );
        assert_eq!(watched.percent, Some(100));
        let unwatched = aggregate(
            ContentType::Movie,
            &[episode("m1", NodeProgressState::Unread)],
        );
        assert_eq!(unwatched.percent, Some(0));
    }

    #[test]
    fn estimate_fills_totals_from_runtime() {
        let runtime = MediaRuntime {
            pages: None,
            duration_min: None,
            ep_count: Some(24),
            ch_count: Some(120),
        };
        assert_eq!(
            estimated_total_units(ContentType::Anime, &runtime),
            Some(24)
        );
        assert_eq!(
            estimated_total_units(ContentType::WebNovel, &runtime),
            Some(120)
        );
        assert_eq!(
            estimated_total_units(ContentType::Book, &runtime),
            None,
            "books use pages, not chapter counts"
        );

        let book_runtime = MediaRuntime {
            pages: Some(432),
            ..runtime
        };
        assert_eq!(
            estimated_total_units(ContentType::Book, &book_runtime),
            Some(432)
        );
    }

    #[test]
    fn with_estimate_only_fills_when_tree_is_empty() {
        let agg = aggregate(ContentType::Novel, &[]).with_estimate(Some(120));
        assert_eq!(agg.total_units, 120);
        assert_eq!(agg.percent, Some(0));

        let nodes = [tick(
            "c1",
            NodeKind::Chapter,
            NodeProgressState::Read,
            None,
            None,
        )];
        let agg = aggregate(ContentType::Novel, &nodes).with_estimate(Some(120));
        assert_eq!(agg.total_units, 1, "node-derived total wins");
        assert_eq!(agg.percent, Some(100));
    }
}
