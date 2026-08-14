//! Status engine (MISSION-024, DOMAIN_MODEL §2.4).
//!
//! Core statuses drive dashboards, stats and auto-transitions; custom statuses
//! are grouped under a core bucket so behavior stays predictable
//! (`status.bucket`). Auto-transition rules (e.g. all episodes watched →
//! completed) are *explicit* and *reversible*: they are pure functions of the
//! progress aggregate, so un-marking a node naturally moves the suggestion back
//! (completed → in progress → planned). The application layer decides whether
//! to apply a suggestion (auto-track mode); this engine only reasons about it.
//!
//! The module is clock-free: callers pass the current date.

use crate::domain::enums::CoreStatus;
use crate::domain::error::DomainError;
use crate::domain::progress::ProgressAggregate;
use crate::domain::tracking::Tracking;
use crate::domain::value_objects::DateOnly;

/// A user-defined status grouped under a core bucket (`status` table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomStatus {
    pub id: String,
    pub name: String,
    /// The core bucket the status behaves like (drives stats/auto-transitions).
    pub bucket: CoreStatus,
    pub sort_order: u32,
}

impl CustomStatus {
    pub fn new(
        id: &str,
        name: &str,
        bucket: CoreStatus,
        sort_order: u32,
    ) -> Result<Self, DomainError> {
        let id = id.trim();
        let name = name.trim();
        if id.is_empty() || name.is_empty() {
            return Err(DomainError::validation(
                "custom status id and name must be non-empty",
            ));
        }
        Ok(Self {
            id: id.into(),
            name: name.into(),
            bucket,
            sort_order,
        })
    }
}

/// Resolve the *effective* core status: a custom status overrides the raw core
/// status when present (REQ-TRACK-003: behavior follows the bucket).
pub fn effective_status(custom: Option<&CustomStatus>, core: CoreStatus) -> CoreStatus {
    custom.map(|c| c.bucket).unwrap_or(core)
}

/// Whether a direct transition between two core statuses is allowed.
///
/// The matrix is deliberately permissive (users may jump between most statuses,
/// matching MAL/AniList UX) with one explicit rule: **Repeat requires prior
/// consumption** — you can only re-read / re-watch something that was started,
/// finished or dropped, never something merely planned or wishlisted.
pub fn can_transition(from: CoreStatus, to: CoreStatus) -> bool {
    if from == to {
        return true;
    }
    match to {
        CoreStatus::Repeat => matches!(
            from,
            CoreStatus::InProgress
                | CoreStatus::Completed
                | CoreStatus::Dropped
                | CoreStatus::Repeat
        ),
        _ => true,
    }
}

/// Apply a status transition, producing a new `Tracking` with the side effects
/// stamped (`today` is caller-supplied; the domain is clock-free):
///   - entering InProgress/Repeat stamps `started_at` when absent,
///   - entering Completed/Dropped stamps `finished_at` when absent,
///   - leaving a terminal bucket clears `finished_at` (reversible),
///   - entering Repeat increments `repeat_count`; leaving Repeat resets it.
///
/// The result is re-validated, so a transition can never produce an invalid
/// record.
pub fn apply_transition(
    tracking: &Tracking,
    to: CoreStatus,
    today: &DateOnly,
) -> Result<Tracking, DomainError> {
    if !can_transition(tracking.core_status, to) {
        return Err(DomainError::validation(format!(
            "invalid status transition {} -> {}",
            tracking.core_status.as_str(),
            to.as_str()
        )));
    }

    let mut next = tracking.clone();

    if next.core_status == CoreStatus::Repeat && to != CoreStatus::Repeat {
        next.repeat_count = 0;
    }
    if to == CoreStatus::Repeat {
        next.repeat_count += 1;
    }

    if matches!(to, CoreStatus::InProgress | CoreStatus::Repeat) && next.started_at.is_none() {
        next.started_at = Some(today.clone());
    }

    if matches!(to, CoreStatus::Completed | CoreStatus::Dropped) {
        if next.finished_at.is_none() {
            next.finished_at = Some(today.clone());
        }
    } else {
        next.finished_at = None;
    }

    next.core_status = to;
    next.validate()?;
    Ok(next)
}

/// The natural status implied by the current progress — the auto-transition
/// rule (explicit, reversible). `None` when there is no node data to reason
/// about (estimates are display-only, never auto-transition triggers).
///
/// Reversible: un-marking the last consumed node moves the suggestion from
/// completed → in progress → planned, never hidden.
pub fn suggest_auto_status(progress: &ProgressAggregate) -> Option<CoreStatus> {
    if progress.total_units == 0 {
        return None;
    }
    if progress.completed_units == progress.total_units {
        Some(CoreStatus::Completed)
    } else if progress.completed_units > 0 {
        Some(CoreStatus::InProgress)
    } else {
        Some(CoreStatus::Planned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::enums::{ContentType, NodeKind, NodeProgressState};
    use crate::domain::progress::{aggregate, NodeTick};
    use crate::domain::value_objects::MediaId;

    fn today() -> DateOnly {
        DateOnly::new("2026-08-11").unwrap()
    }

    fn tracking(status: CoreStatus) -> Tracking {
        Tracking {
            media_id: MediaId::new("m-1").unwrap(),
            core_status: status,
            custom_status_id: None,
            started_at: None,
            finished_at: None,
            repeat_count: 0,
            current_node_id: None,
            current_position: Some(12),
            auto_track: true,
            updated_at: "2026-08-11".into(),
        }
    }

    #[test]
    fn status_classification() {
        assert!(CoreStatus::Completed.is_terminal());
        assert!(CoreStatus::Dropped.is_terminal());
        assert!(!CoreStatus::InProgress.is_terminal());

        assert!(CoreStatus::InProgress.is_active());
        assert!(CoreStatus::Repeat.is_active());
        assert!(!CoreStatus::OnHold.is_active());

        assert!(CoreStatus::Planned.is_not_started());
        assert!(CoreStatus::Wishlist.is_not_started());
        assert!(!CoreStatus::OnHold.is_not_started());
    }

    #[test]
    fn repeat_requires_prior_consumption() {
        assert!(can_transition(CoreStatus::Completed, CoreStatus::Repeat));
        assert!(can_transition(CoreStatus::Dropped, CoreStatus::Repeat));
        assert!(can_transition(CoreStatus::InProgress, CoreStatus::Repeat));
        assert!(!can_transition(CoreStatus::Planned, CoreStatus::Repeat));
        assert!(!can_transition(CoreStatus::Wishlist, CoreStatus::Repeat));
        assert!(!can_transition(CoreStatus::OnHold, CoreStatus::Repeat));

        let err = apply_transition(&tracking(CoreStatus::Planned), CoreStatus::Repeat, &today());
        assert!(err.is_err());
    }

    #[test]
    fn most_direct_jumps_are_allowed() {
        for from in CoreStatus::ALL {
            for to in CoreStatus::ALL {
                if *to == CoreStatus::Repeat
                    && matches!(
                        *from,
                        CoreStatus::Planned | CoreStatus::Wishlist | CoreStatus::OnHold
                    )
                {
                    assert!(!can_transition(*from, *to), "{from:?} -> {to:?}");
                } else {
                    assert!(can_transition(*from, *to), "{from:?} -> {to:?}");
                }
            }
        }
    }

    #[test]
    fn starting_stamps_started_at() {
        let started = apply_transition(
            &tracking(CoreStatus::Planned),
            CoreStatus::InProgress,
            &today(),
        )
        .unwrap();
        assert_eq!(started.started_at, Some(today()));
        assert!(started.finished_at.is_none());
    }

    #[test]
    fn finishing_stamps_finished_at() {
        let mut t = tracking(CoreStatus::InProgress);
        t.started_at = Some(DateOnly::new("2026-08-01").unwrap());
        let finished = apply_transition(&t, CoreStatus::Completed, &today()).unwrap();
        assert_eq!(
            finished.started_at,
            Some(DateOnly::new("2026-08-01").unwrap())
        );
        assert_eq!(finished.finished_at, Some(today()));
    }

    #[test]
    fn leaving_terminal_clears_finished_at() {
        let mut t = tracking(CoreStatus::Completed);
        t.started_at = Some(DateOnly::new("2026-08-01").unwrap());
        t.finished_at = Some(today());
        let held = apply_transition(&t, CoreStatus::OnHold, &today()).unwrap();
        assert_eq!(held.core_status, CoreStatus::OnHold);
        assert!(
            held.finished_at.is_none(),
            "reversible: finished_at cleared"
        );
        assert_eq!(held.started_at, Some(DateOnly::new("2026-08-01").unwrap()));
    }

    #[test]
    fn repeat_count_increments_on_entry_and_resets_on_exit() {
        let mut t = tracking(CoreStatus::Completed);
        t.finished_at = Some(today());

        let repeat = apply_transition(&t, CoreStatus::Repeat, &today()).unwrap();
        assert_eq!(repeat.repeat_count, 1);
        assert!(
            repeat.finished_at.is_none(),
            "a new run clears the old finish"
        );

        let again = apply_transition(&repeat, CoreStatus::Repeat, &today()).unwrap();
        assert_eq!(again.repeat_count, 2);

        let in_progress = apply_transition(&again, CoreStatus::InProgress, &today()).unwrap();
        assert_eq!(
            in_progress.repeat_count, 0,
            "fresh cycle after leaving repeat"
        );
    }

    #[test]
    fn transition_never_produces_invalid_tracking() {
        // An inconsistent record cannot slip through: targeting a terminal
        // bucket keeps the bad finished_at, so validate() rejects it.
        let mut t = tracking(CoreStatus::InProgress);
        t.started_at = Some(DateOnly::new("2026-08-11").unwrap());
        t.finished_at = Some(DateOnly::new("2026-08-10").unwrap()); // finish before start
        assert!(apply_transition(&t, CoreStatus::Completed, &today()).is_err());

        // Every allowed transition out of a valid record stays valid.
        for from in CoreStatus::ALL {
            for to in CoreStatus::ALL {
                if !can_transition(*from, *to) {
                    continue;
                }
                let valid = apply_transition(&tracking(*from), *to, &today()).unwrap();
                assert!(valid.validate().is_ok(), "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn custom_status_resolution_and_validation() {
        let custom =
            CustomStatus::new("marathoning", "Marathoning", CoreStatus::InProgress, 25).unwrap();
        assert_eq!(
            effective_status(Some(&custom), CoreStatus::Planned),
            CoreStatus::InProgress
        );
        assert_eq!(
            effective_status(None, CoreStatus::Planned),
            CoreStatus::Planned
        );

        assert!(CustomStatus::new("", "Empty", CoreStatus::Planned, 1).is_err());
        assert!(CustomStatus::new("id", "  ", CoreStatus::Planned, 1).is_err());
    }

    fn tick(id: &str, kind: NodeKind, state: NodeProgressState) -> NodeTick {
        NodeTick {
            id: id.into(),
            kind,
            state,
            page_count: None,
            duration_min: None,
        }
    }

    #[test]
    fn auto_status_follows_progress_and_is_reversible() {
        let episodes = [
            tick("e1", NodeKind::Episode, NodeProgressState::Watched),
            tick("e2", NodeKind::Episode, NodeProgressState::Watched),
            tick("e3", NodeKind::Episode, NodeProgressState::Unread),
        ];
        let partial = aggregate(ContentType::Anime, &episodes);
        assert_eq!(suggest_auto_status(&partial), Some(CoreStatus::InProgress));

        let watched = [
            tick("e1", NodeKind::Episode, NodeProgressState::Watched),
            tick("e2", NodeKind::Episode, NodeProgressState::Watched),
            tick("e3", NodeKind::Episode, NodeProgressState::Watched),
        ];
        let complete = aggregate(ContentType::Anime, &watched);
        assert_eq!(suggest_auto_status(&complete), Some(CoreStatus::Completed));

        // Reversible: un-marking the last episode moves the suggestion back.
        let reverted = aggregate(ContentType::Anime, &episodes);
        assert_eq!(suggest_auto_status(&reverted), Some(CoreStatus::InProgress));

        let none = aggregate(
            ContentType::Anime,
            &[tick("e1", NodeKind::Episode, NodeProgressState::Unread)],
        );
        assert_eq!(suggest_auto_status(&none), Some(CoreStatus::Planned));
    }

    #[test]
    fn auto_status_is_none_without_node_data() {
        let empty = aggregate(ContentType::Anime, &[]);
        assert_eq!(suggest_auto_status(&empty), None, "no data, no suggestion");
    }
}
