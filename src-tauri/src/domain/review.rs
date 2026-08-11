//! The `Review` aggregate (MISSION-022, DOMAIN_MODEL §2.5).
//!
//! User-owned data: personal rating, review text, notes, favorite. External
//! ratings/reviews are metadata on `Media` and never land here (invariant from
//! DOMAIN_MODEL §6). One review per media in the MVP.

use std::str::FromStr;

use crate::domain::error::DomainError;
use crate::domain::value_objects::{DateOnly, MediaId, Rating};

/// A media's user review (one per media).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub media_id: MediaId,
    pub rating: Option<Rating>,
    pub review: Option<String>,
    pub short_review: Option<String>,
    pub notes: Option<String>,
    pub favorite: bool,
    pub is_spoiler: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Review {
    /// Validate the review's invariants.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.created_at.trim().is_empty() || self.updated_at.trim().is_empty() {
            return Err(DomainError::validation(
                "review timestamps must not be empty",
            ));
        }

        // updated_at >= created_at when both are ISO dates.
        if let (Ok(created), Ok(updated)) = (
            DateOnly::from_str(&self.created_at),
            DateOnly::from_str(&self.updated_at),
        ) {
            if updated < created {
                return Err(DomainError::validation(format!(
                    "review updated_at {} is before created_at {}",
                    updated.as_str(),
                    created.as_str()
                )));
            }
        }

        // A spoiler flag is only meaningful when there is text to spoil.
        let has_text = [&self.review, &self.short_review, &self.notes]
            .iter()
            .any(|text| matches!(text.as_deref().map(str::trim), Some(t) if !t.is_empty()));
        if self.is_spoiler && !has_text {
            return Err(DomainError::validation(
                "is_spoiler set but there is no review, short review or note to spoil",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review() -> Review {
        Review {
            media_id: MediaId::new("m-1").unwrap(),
            rating: Some(Rating::new(9).unwrap()),
            review: Some("A sweeping epic".into()),
            short_review: None,
            notes: None,
            favorite: true,
            is_spoiler: false,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        }
    }

    #[test]
    fn valid_review_passes() {
        review().validate().expect("valid review");
    }

    #[test]
    fn timestamp_order_enforced() {
        let mut r = review();
        r.updated_at = "2025-12-31".into();
        assert!(r.validate().is_err());
    }

    #[test]
    fn spoiler_requires_text() {
        let mut r = review();
        r.is_spoiler = true;
        r.review = None;
        r.short_review = None;
        r.notes = None;
        assert!(r.validate().is_err(), "spoiler without text rejected");
        r.notes = Some("the twist".into());
        assert!(r.validate().is_ok(), "notes can carry the spoiler too");
    }

    #[test]
    fn rating_is_optional_and_bounded() {
        let mut r = review();
        r.rating = None;
        assert!(r.validate().is_ok());
        assert!(Rating::new(10).is_ok());
        assert!(Rating::new(0).is_err());
    }
}
