//! The `Review` aggregate (MISSION-022, DOMAIN_MODEL §2.5).
//!
//! User-owned data: personal rating, review text, notes, favorite, and
//! StoryGraph-style metadata — mood, pace and content warnings (MISSION-079).
//! External ratings/reviews are metadata on `Media` and never land here
//! (invariant from DOMAIN_MODEL §6). One review per media in the MVP.
//!
//! Moods, paces and content warnings come from fixed vocabularies; unknown
//! keys are rejected at the boundary (MISSION-079). Content warnings carry an
//! optional acknowledgment timestamp — an acknowledgment is only meaningful
//! while the current warning set is non-empty ("acknowledged metadata, never
//! forced").

use std::str::FromStr;

use crate::domain::error::DomainError;
use crate::domain::value_objects::{DateOnly, MediaId, Rating};

/// The fixed mood vocabulary (StoryGraph-inspired, MISSION-079).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mood {
    Adventurous,
    Dark,
    Emotional,
    Funny,
    Hopeful,
    Inspiring,
    Informative,
    Lighthearted,
    Mysterious,
    Romantic,
    Sad,
    Tense,
}

impl Mood {
    /// Every known mood, in canonical order (the order stored and sorted).
    pub const ALL: &'static [Mood] = &[
        Mood::Adventurous,
        Mood::Dark,
        Mood::Emotional,
        Mood::Funny,
        Mood::Hopeful,
        Mood::Inspiring,
        Mood::Informative,
        Mood::Lighthearted,
        Mood::Mysterious,
        Mood::Romantic,
        Mood::Sad,
        Mood::Tense,
    ];

    /// The stable storage key for this mood.
    pub fn as_str(self) -> &'static str {
        match self {
            Mood::Adventurous => "adventurous",
            Mood::Dark => "dark",
            Mood::Emotional => "emotional",
            Mood::Funny => "funny",
            Mood::Hopeful => "hopeful",
            Mood::Inspiring => "inspiring",
            Mood::Informative => "informative",
            Mood::Lighthearted => "lighthearted",
            Mood::Mysterious => "mysterious",
            Mood::Romantic => "romantic",
            Mood::Sad => "sad",
            Mood::Tense => "tense",
        }
    }
}

impl FromStr for Mood {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Mood::ALL
            .iter()
            .find(|mood| mood.as_str() == s)
            .copied()
            .ok_or_else(|| DomainError::validation(format!("unknown mood: {s}")))
    }
}

/// The fixed pace vocabulary (MISSION-079).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    Slow,
    Medium,
    Fast,
}

impl Pace {
    /// Every known pace, in canonical order.
    pub const ALL: &'static [Pace] = &[Pace::Slow, Pace::Medium, Pace::Fast];

    /// The stable storage key for this pace.
    pub fn as_str(self) -> &'static str {
        match self {
            Pace::Slow => "slow",
            Pace::Medium => "medium",
            Pace::Fast => "fast",
        }
    }
}

impl FromStr for Pace {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Pace::ALL
            .iter()
            .find(|pace| pace.as_str() == s)
            .copied()
            .ok_or_else(|| DomainError::validation(format!("unknown pace: {s}")))
    }
}

/// The fixed content-warning vocabulary (MISSION-079).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentWarning {
    Violence,
    Gore,
    SexualContent,
    StrongLanguage,
    SelfHarm,
    Suicide,
    DrugUse,
    Alcohol,
    Death,
    Abuse,
    Bullying,
    AnimalDeath,
    Racism,
    Transphobia,
}

impl ContentWarning {
    /// Every known content warning, in canonical order.
    pub const ALL: &'static [ContentWarning] = &[
        ContentWarning::Violence,
        ContentWarning::Gore,
        ContentWarning::SexualContent,
        ContentWarning::StrongLanguage,
        ContentWarning::SelfHarm,
        ContentWarning::Suicide,
        ContentWarning::DrugUse,
        ContentWarning::Alcohol,
        ContentWarning::Death,
        ContentWarning::Abuse,
        ContentWarning::Bullying,
        ContentWarning::AnimalDeath,
        ContentWarning::Racism,
        ContentWarning::Transphobia,
    ];

    /// The stable storage key for this content warning.
    pub fn as_str(self) -> &'static str {
        match self {
            ContentWarning::Violence => "violence",
            ContentWarning::Gore => "gore",
            ContentWarning::SexualContent => "sexual_content",
            ContentWarning::StrongLanguage => "strong_language",
            ContentWarning::SelfHarm => "self_harm",
            ContentWarning::Suicide => "suicide",
            ContentWarning::DrugUse => "drug_use",
            ContentWarning::Alcohol => "alcohol",
            ContentWarning::Death => "death",
            ContentWarning::Abuse => "abuse",
            ContentWarning::Bullying => "bullying",
            ContentWarning::AnimalDeath => "animal_death",
            ContentWarning::Racism => "racism",
            ContentWarning::Transphobia => "transphobia",
        }
    }
}

impl FromStr for ContentWarning {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ContentWarning::ALL
            .iter()
            .find(|warning| warning.as_str() == s)
            .copied()
            .ok_or_else(|| DomainError::validation(format!("unknown content warning: {s}")))
    }
}

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
    pub moods: Vec<Mood>,
    pub pace: Option<Pace>,
    pub content_warnings: Vec<ContentWarning>,
    /// When the user last acknowledged the *current* content-warning set.
    pub warnings_acknowledged_at: Option<String>,
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

        // Moods and content warnings are stored deduplicated (MISSION-079).
        if has_duplicates(&self.moods) {
            return Err(DomainError::validation("moods must not contain duplicates"));
        }
        if has_duplicates(&self.content_warnings) {
            return Err(DomainError::validation(
                "content warnings must not contain duplicates",
            ));
        }

        // An acknowledgment is only meaningful for a non-empty warning set.
        if let Some(stamp) = &self.warnings_acknowledged_at {
            if stamp.trim().is_empty() {
                return Err(DomainError::validation(
                    "warnings acknowledgment timestamp must not be empty",
                ));
            }
            if self.content_warnings.is_empty() {
                return Err(DomainError::validation(
                    "warnings acknowledged but there are no content warnings",
                ));
            }
        }

        Ok(())
    }
}

fn has_duplicates<T: PartialEq + Copy>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
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
            moods: vec![],
            pace: None,
            content_warnings: vec![],
            warnings_acknowledged_at: None,
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

    #[test]
    fn metadata_fields_parse_from_their_keys() {
        for mood in Mood::ALL {
            assert_eq!(Mood::from_str(mood.as_str()).unwrap(), *mood);
        }
        for pace in Pace::ALL {
            assert_eq!(Pace::from_str(pace.as_str()).unwrap(), *pace);
        }
        for warning in ContentWarning::ALL {
            assert_eq!(
                ContentWarning::from_str(warning.as_str()).unwrap(),
                *warning
            );
        }
        assert!(Mood::from_str("nope").is_err());
        assert!(Pace::from_str("brisk").is_err());
        assert!(ContentWarning::from_str("nope").is_err());
    }

    #[test]
    fn acknowledged_warnings_require_a_warning_set() {
        let mut r = review();
        r.warnings_acknowledged_at = Some("2026-02-01T00:00:00Z".into());
        assert!(
            r.validate().is_err(),
            "acknowledged with no warnings rejected"
        );
        r.content_warnings = vec![ContentWarning::Violence];
        assert!(r.validate().is_ok(), "acknowledged with warnings passes");
        r.warnings_acknowledged_at = Some("   ".into());
        assert!(r.validate().is_err(), "blank stamp rejected");
    }

    #[test]
    fn duplicate_moods_and_warnings_rejected() {
        let mut r = review();
        r.moods = vec![Mood::Dark, Mood::Dark];
        assert!(r.validate().is_err(), "duplicate moods rejected");
        r.moods = vec![Mood::Dark];
        r.content_warnings = vec![ContentWarning::Gore, ContentWarning::Gore];
        assert!(r.validate().is_err(), "duplicate warnings rejected");
    }
}
