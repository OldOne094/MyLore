//! The `Media` entity (MISSION-022, DOMAIN_MODEL §2.1).
//!
//! Owns *metadata* only — user-owned data (tracking, review, tags, collections)
//! lives on separate aggregates so a metadata refresh can never clobber it.
//! Invariants enforced by `validate`:
//!   - the media always has a title (guaranteed by `Title::new`),
//!   - no two external ids share a provider (media_external_id UNIQUE),
//!   - a relation never points at the media itself (from_id <> to_id).

use crate::domain::enums::{ContentType, MediaRelationKind, MediaStatus, PersonRole};
use crate::domain::error::DomainError;
use crate::domain::value_objects::{
    DateOnly, ExternalId, LanguageCode, MediaId, ProviderId, Title,
};

/// Optional aggregate runtime counters (media.pages/duration_min/ep_count/ch_count).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaRuntime {
    pub pages: Option<u32>,
    pub duration_min: Option<u32>,
    pub ep_count: Option<u32>,
    pub ch_count: Option<u32>,
}

/// A person credited on the media (author/artist/studio/…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonCredit {
    pub person_id: String,
    pub name: String,
    pub role: PersonRole,
}

/// A directed relationship to another media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRelation {
    pub to_id: MediaId,
    pub kind: MediaRelationKind,
}

/// The full media metadata aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    pub id: MediaId,
    pub content_type: ContentType,
    pub format: Option<String>,
    pub title: Title,
    pub synopsis: Option<String>,
    pub status: MediaStatus,
    pub start_date: Option<DateOnly>,
    pub end_date: Option<DateOnly>,
    pub release_year: Option<u16>,
    pub language: Option<LanguageCode>,
    pub country: Option<String>,
    pub content_rating: Option<String>,
    pub runtime: MediaRuntime,
    pub people: Vec<PersonCredit>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub external_ids: Vec<ExternalId>,
    pub relations: Vec<MediaRelation>,
    pub provider: Option<ProviderId>,
    pub provider_url: Option<String>,
    pub metadata_refreshed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Media {
    /// Validate the aggregate's cross-field invariants.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.created_at.trim().is_empty() || self.updated_at.trim().is_empty() {
            return Err(DomainError::validation(
                "media timestamps must not be empty",
            ));
        }

        // A media may not hold two external ids for the same provider.
        let mut providers = std::collections::HashSet::new();
        for external_id in &self.external_ids {
            if !providers.insert(external_id.provider().as_str().to_string()) {
                return Err(DomainError::validation(format!(
                    "media {} holds more than one external id for provider {}",
                    self.id,
                    external_id.provider().as_str()
                )));
            }
        }

        // A relation must point at a different media.
        for relation in &self.relations {
            if relation.to_id == self.id {
                return Err(DomainError::validation(format!(
                    "media {} cannot be related to itself",
                    self.id
                )));
            }
        }

        // Both dates present ⇒ start <= end.
        if let (Some(start), Some(end)) = (&self.start_date, &self.end_date) {
            if start > end {
                return Err(DomainError::validation(format!(
                    "media {} start date {} is after end date {}",
                    self.id,
                    start.as_str(),
                    end.as_str()
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media() -> Media {
        Media {
            id: MediaId::new("m-1").unwrap(),
            content_type: ContentType::Novel,
            format: Some("light_novel".into()),
            title: Title::new("Sword of the Dawn", None, vec![]).unwrap(),
            synopsis: None,
            status: MediaStatus::Ongoing,
            start_date: Some(DateOnly::new("2025-01-01").unwrap()),
            end_date: None,
            release_year: Some(2025),
            language: Some(LanguageCode::new("ja").unwrap()),
            country: None,
            content_rating: None,
            runtime: MediaRuntime::default(),
            people: Vec::new(),
            genres: vec!["fantasy".into()],
            tags: Vec::new(),
            external_ids: Vec::new(),
            relations: Vec::new(),
            provider: Some(ProviderId::new("anilist").unwrap()),
            provider_url: None,
            metadata_refreshed_at: None,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        }
    }

    #[test]
    fn valid_media_passes() {
        media().validate().expect("valid media");
    }

    #[test]
    fn duplicate_provider_external_ids_rejected() {
        let mut m = media();
        m.external_ids
            .push(ExternalId::new(ProviderId::new("anilist").unwrap(), "42", None).unwrap());
        m.external_ids
            .push(ExternalId::new(ProviderId::new("anilist").unwrap(), "43", None).unwrap());
        assert!(m.validate().is_err());
    }

    #[test]
    fn self_relation_rejected() {
        let mut m = media();
        m.relations.push(MediaRelation {
            to_id: MediaId::new("m-1").unwrap(),
            kind: MediaRelationKind::Sequel,
        });
        assert!(m.validate().is_err());
    }

    #[test]
    fn date_order_enforced() {
        let mut m = media();
        m.start_date = Some(DateOnly::new("2026-06-01").unwrap());
        m.end_date = Some(DateOnly::new("2025-01-01").unwrap());
        assert!(m.validate().is_err());
    }

    #[test]
    fn blank_timestamps_rejected() {
        let mut m = media();
        m.updated_at = "".into();
        assert!(m.validate().is_err());
    }
}
