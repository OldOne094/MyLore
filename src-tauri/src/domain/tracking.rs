//! The `Tracking` aggregate (MISSION-022, DOMAIN_MODEL §2.3).
//!
//! Per-media user state. Aggregate progress is *derived* from node states and
//! is never stored here. Status semantics and auto-transitions are the status
//! engine's job (MISSION-024); this entity only persists the record and guards
//! its shape.

use crate::domain::enums::CoreStatus;
use crate::domain::error::DomainError;
use crate::domain::value_objects::{DateOnly, MediaId};

/// Per-media tracking state (one row per media in the MVP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracking {
    pub media_id: MediaId,
    pub core_status: CoreStatus,
    pub custom_status_id: Option<String>,
    pub started_at: Option<DateOnly>,
    pub finished_at: Option<DateOnly>,
    pub repeat_count: u32,
    pub current_node_id: Option<String>,
    pub current_position: Option<u32>,
    pub updated_at: String,
}

impl Tracking {
    /// Validate the record's invariants.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.updated_at.trim().is_empty() {
            return Err(DomainError::validation(
                "tracking updated_at must not be empty",
            ));
        }

        // A repeat (re-read/re-watch) is the only context where `repeat_count`
        // is meaningful; guard against nonsensical values on other statuses.
        if self.repeat_count > 0 && self.core_status != CoreStatus::Repeat {
            return Err(DomainError::validation(format!(
                "repeat_count is {} but core_status is {}",
                self.repeat_count,
                self.core_status.as_str()
            )));
        }

        // finished implies a terminal bucket (completed/dropped).
        if self.finished_at.is_some()
            && !matches!(
                self.core_status,
                CoreStatus::Completed | CoreStatus::Dropped
            )
        {
            return Err(DomainError::validation(format!(
                "finished_at set while core_status is {}",
                self.core_status.as_str()
            )));
        }

        // finish is not before start.
        if let (Some(start), Some(finish)) = (&self.started_at, &self.finished_at) {
            if start > finish {
                return Err(DomainError::validation(format!(
                    "finished_at {} is before started_at {}",
                    finish.as_str(),
                    start.as_str()
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracking(status: CoreStatus) -> Tracking {
        Tracking {
            media_id: MediaId::new("m-1").unwrap(),
            core_status: status,
            custom_status_id: None,
            started_at: Some(DateOnly::new("2026-01-01").unwrap()),
            finished_at: None,
            repeat_count: 0,
            current_node_id: None,
            current_position: Some(12),
            updated_at: "2026-01-01".into(),
        }
    }

    #[test]
    fn repeat_count_only_for_repeat_status() {
        assert!(tracking(CoreStatus::Repeat).validate().is_ok());
        let mut t = tracking(CoreStatus::Completed);
        t.repeat_count = 2;
        assert!(t.validate().is_err());
        let mut t = tracking(CoreStatus::Repeat);
        t.repeat_count = 2;
        assert!(t.validate().is_ok());
    }

    #[test]
    fn finished_requires_terminal_status() {
        let mut t = tracking(CoreStatus::Completed);
        t.finished_at = Some(DateOnly::new("2026-02-01").unwrap());
        assert!(t.validate().is_ok());
        let mut t = tracking(CoreStatus::Dropped);
        t.finished_at = Some(DateOnly::new("2026-02-01").unwrap());
        assert!(t.validate().is_ok());
        let mut t = tracking(CoreStatus::InProgress);
        t.finished_at = Some(DateOnly::new("2026-02-01").unwrap());
        assert!(t.validate().is_err());
    }

    #[test]
    fn finish_not_before_start() {
        let mut t = tracking(CoreStatus::Completed);
        t.started_at = Some(DateOnly::new("2026-03-01").unwrap());
        t.finished_at = Some(DateOnly::new("2026-02-01").unwrap());
        assert!(t.validate().is_err());
    }
}
