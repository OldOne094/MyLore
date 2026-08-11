//! `ContentNode` and `NodeProgress` (MISSION-022, DOMAIN_MODEL §2.2–2.3).
//!
//! One node type models every hierarchy (seasons→episodes, volumes→chapters,
//! flat media) via `kind + parent + position`. Progress is per-node and the
//! aggregate is derived, never stored.
//!
//! Invariants:
//!   - a node's `position` is 1-based within its parent,
//!   - a consumed state (`read`/`watched`) requires `read_at`,
//!   - an unconsumed state (`unread`/`skipped`) forbids `read_at`,
//!   - ratings are 1..10 (`Rating`),
//!   - the cross-row invariant "a parent must belong to the same media" is
//!     enforced at the repository boundary (`infrastructure::content_node`).

use crate::domain::enums::{NodeKind, NodeProgressState};
use crate::domain::error::DomainError;
use crate::domain::value_objects::{DateOnly, MediaId, Rating};

/// A row of the generic content-node tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentNode {
    pub id: String,
    pub media_id: MediaId,
    pub parent_id: Option<String>,
    pub kind: NodeKind,
    pub position: u32,
    pub number: Option<String>,
    pub title: Option<String>,
    pub release_date: Option<DateOnly>,
    pub duration_min: Option<u32>,
    pub page_count: Option<u32>,
    pub synopsis: Option<String>,
    pub external_id: Option<String>,
    pub is_special: bool,
    pub created_at: String,
}

impl ContentNode {
    /// Validate this node's own invariants.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.id.trim().is_empty() {
            return Err(DomainError::validation("node id must not be empty"));
        }
        if self.position == 0 {
            return Err(DomainError::validation(format!(
                "node {} position must be 1-based, got 0",
                self.id
            )));
        }
        if self.created_at.trim().is_empty() {
            return Err(DomainError::validation("node created_at must not be empty"));
        }
        Ok(())
    }
}

/// Per-node user progress (one row per node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeProgress {
    pub node_id: String,
    pub state: NodeProgressState,
    pub read_at: Option<DateOnly>,
    pub note: Option<String>,
    pub rating: Option<Rating>,
    pub updated_at: String,
}

impl NodeProgress {
    /// Validate the state/read_at coupling invariant.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.node_id.trim().is_empty() {
            return Err(DomainError::validation("node_id must not be empty"));
        }
        if self.updated_at.trim().is_empty() {
            return Err(DomainError::validation(
                "progress updated_at must not be empty",
            ));
        }
        match self.state {
            NodeProgressState::Read | NodeProgressState::Watched if self.read_at.is_none() => {
                return Err(DomainError::validation(format!(
                    "state {:?} requires read_at on node {}",
                    self.state, self.node_id
                )));
            }
            NodeProgressState::Unread | NodeProgressState::Skipped if self.read_at.is_some() => {
                return Err(DomainError::validation(format!(
                    "state {:?} must not carry read_at on node {}",
                    self.state, self.node_id
                )));
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> ContentNode {
        ContentNode {
            id: "n-1".into(),
            media_id: MediaId::new("m-1").unwrap(),
            parent_id: None,
            kind: NodeKind::Chapter,
            position: 1,
            number: Some("1".into()),
            title: None,
            release_date: None,
            duration_min: None,
            page_count: Some(20),
            synopsis: None,
            external_id: None,
            is_special: false,
            created_at: "2026-01-01".into(),
        }
    }

    #[test]
    fn node_position_is_1_based() {
        let mut n = node();
        n.position = 0;
        assert!(n.validate().is_err());
        n.position = 1;
        assert!(n.validate().is_ok());
    }

    fn progress(state: NodeProgressState, read_at: Option<DateOnly>) -> NodeProgress {
        NodeProgress {
            node_id: "n-1".into(),
            state,
            read_at,
            note: None,
            rating: None,
            updated_at: "2026-01-02".into(),
        }
    }

    #[test]
    fn consumed_states_require_read_at() {
        assert!(progress(NodeProgressState::Read, None).validate().is_err());
        assert!(progress(NodeProgressState::Watched, None)
            .validate()
            .is_err());
        assert!(progress(
            NodeProgressState::Read,
            Some(DateOnly::new("2026-01-02").unwrap())
        )
        .validate()
        .is_ok());
        assert!(progress(
            NodeProgressState::Watched,
            Some(DateOnly::new("2026-01-02").unwrap())
        )
        .validate()
        .is_ok());
    }

    #[test]
    fn unconsumed_states_forbid_read_at() {
        assert!(progress(
            NodeProgressState::Unread,
            Some(DateOnly::new("2026-01-02").unwrap())
        )
        .validate()
        .is_err());
        assert!(progress(
            NodeProgressState::Skipped,
            Some(DateOnly::new("2026-01-02").unwrap())
        )
        .validate()
        .is_err());
        assert!(progress(NodeProgressState::Unread, None).validate().is_ok());
        assert!(progress(NodeProgressState::Skipped, None)
            .validate()
            .is_ok());
        assert!(progress(NodeProgressState::Partial, None)
            .validate()
            .is_ok());
        assert!(progress(
            NodeProgressState::Partial,
            Some(DateOnly::new("2026-01-02").unwrap())
        )
        .validate()
        .is_ok());
    }

    #[test]
    fn rating_is_optional_and_bounded() {
        let mut p = progress(
            NodeProgressState::Read,
            Some(DateOnly::new("2026-01-02").unwrap()),
        );
        assert!(p.validate().is_ok(), "no rating is fine");
        p.rating = Some(Rating::new(9).unwrap());
        assert!(p.validate().is_ok());
        assert!(Rating::new(0).is_err());
        assert!(Rating::new(11).is_err());
    }
}
