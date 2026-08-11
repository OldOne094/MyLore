//! Domain value objects (MISSION-022, DOMAIN_MODEL §5).
//!
//! Values are immutable by convention (only `&self` accessors expose fields
//! that were validated at construction). Each constructor enforces its
//! invariant and returns `Result`, so an invalid value cannot be constructed.

use std::str::FromStr;

use crate::domain::error::DomainError;

/// Internal UUID-style identifier for a media.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaId(String);

impl MediaId {
    /// Create an id; empty/whitespace ids are rejected.
    pub fn new(id: impl Into<String>) -> Result<Self, DomainError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(DomainError::validation("media id must not be empty"));
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MediaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A user rating, constrained to 1..=10 (review.rating CHECK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rating(u8);

impl Rating {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 10;

    /// Create a rating; values outside 1..=10 are rejected.
    pub fn new(value: i64) -> Result<Self, DomainError> {
        if (Self::MIN as i64..=Self::MAX as i64).contains(&value) {
            Ok(Self(value as u8))
        } else {
            Err(DomainError::validation(format!(
                "rating must be between {} and {}, got {value}",
                Self::MIN,
                Self::MAX
            )))
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// A calendar date in ISO `YYYY-MM-DD` form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateOnly(String);

impl DateOnly {
    /// Create a date; must be a well-formed `YYYY-MM-DD` value.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        Self::validate(&value)?;
        Ok(Self(value))
    }

    fn validate(value: &str) -> Result<(), DomainError> {
        let bytes = value.as_bytes();
        let invalid = || DomainError::validation(format!("invalid ISO date: {value:?}"));
        if bytes.len() != 10 {
            return Err(invalid());
        }
        let parse2 = |i: usize| {
            let s = value.get(i..i + 2)?;
            s.parse::<u8>().ok()
        };
        let (year, month, day) = (
            value.get(0..4).and_then(|s| s.parse::<u16>().ok()),
            parse2(5),
            parse2(8),
        );
        match (year, month, day) {
            (Some(_), Some(1..=12), Some(1..=31)) => Ok(()),
            _ => Err(invalid()),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for DateOnly {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// An ISO 639 language code (2–3 lowercase letters).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageCode(String);

impl LanguageCode {
    /// Create a language code; must be 2–3 ASCII lowercase letters.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let valid = (2..=3).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_lowercase());
        if valid {
            Ok(Self(value))
        } else {
            Err(DomainError::validation(format!(
                "invalid language code: {value:?}"
            )))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A metadata-provider identifier (e.g. `anilist`, `tmdb`, `openlibrary`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    /// Create a provider id; must be `[a-z0-9_]+` and non-empty.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if valid {
            Ok(Self(value))
        } else {
            Err(DomainError::validation(format!(
                "invalid provider id: {value:?}"
            )))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The title aggregate of a media (DOMAIN_MODEL §2.1).
///
/// Invariant: the display (`main`) title is always present; alternatives are
/// optional and unique (case-insensitively) relative to each other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title {
    main: String,
    original: Option<String>,
    alternatives: Vec<String>,
}

impl Title {
    /// Create a title; the main title must be non-blank.
    pub fn new(
        main: impl Into<String>,
        original: Option<String>,
        alternatives: Vec<String>,
    ) -> Result<Self, DomainError> {
        let main = main.into();
        if main.trim().is_empty() {
            return Err(DomainError::validation(
                "a media must have at least one title (title_main)",
            ));
        }
        let main_lower = main.to_lowercase();
        let mut seen = vec![main_lower.clone()];
        if let Some(original) = &original {
            let lower = original.to_lowercase();
            if lower == main_lower {
                return Err(DomainError::validation(format!(
                    "original title duplicates the main title: {original:?}"
                )));
            }
            seen.push(lower);
        }
        for alt in &alternatives {
            let lower = alt.to_lowercase();
            if lower.trim().is_empty() {
                return Err(DomainError::validation(
                    "alternative titles must not be blank",
                ));
            }
            if seen.contains(&lower) {
                return Err(DomainError::validation(format!("duplicate title: {alt:?}")));
            }
            seen.push(lower);
        }
        Ok(Self {
            main,
            original,
            alternatives,
        })
    }

    pub fn main(&self) -> &str {
        &self.main
    }

    pub fn original(&self) -> Option<&str> {
        self.original.as_deref()
    }

    pub fn alternatives(&self) -> &[String] {
        &self.alternatives
    }

    /// The main title plus every alternative, for display/search indexing.
    pub fn all(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.main.as_str())
            .chain(self.original.iter().map(String::as_str))
            .chain(self.alternatives.iter().map(String::as_str))
    }
}

/// An external identity on a provider (media_external_id).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalId {
    provider: ProviderId,
    value: String,
    url: Option<String>,
}

impl ExternalId {
    /// Create an external id; the provider value must be non-blank.
    pub fn new(
        provider: ProviderId,
        value: impl Into<String>,
        url: Option<String>,
    ) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::validation(format!(
                "external id value must not be empty for provider {}",
                provider.as_str()
            )));
        }
        Ok(Self {
            provider,
            value,
            url,
        })
    }

    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_id_rejects_blanks() {
        assert!(MediaId::new("").is_err());
        assert!(MediaId::new("  ").is_err());
        assert_eq!(MediaId::new("m-1").unwrap().as_str(), "m-1");
    }

    #[test]
    fn rating_bounds() {
        assert_eq!(Rating::new(1).unwrap().get(), 1);
        assert_eq!(Rating::new(10).unwrap().get(), 10);
        assert!(Rating::new(0).is_err());
        assert!(Rating::new(11).is_err());
    }

    #[test]
    fn date_only_format() {
        assert_eq!(DateOnly::new("2026-01-31").unwrap().as_str(), "2026-01-31");
        for bad in [
            "2026-1-1",
            "26-01-01",
            "2026-13-01",
            "2026-00-01",
            "2026-01-32",
            "not-a-date",
            "",
        ] {
            assert!(DateOnly::new(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn language_and_provider_codes() {
        assert_eq!(LanguageCode::new("ar").unwrap().as_str(), "ar");
        assert_eq!(LanguageCode::new("jpn").unwrap().as_str(), "jpn");
        assert!(LanguageCode::new("english").is_err());
        assert!(LanguageCode::new("EN").is_err());

        assert_eq!(ProviderId::new("anilist").unwrap().as_str(), "anilist");
        assert_eq!(
            ProviderId::new("google_books").unwrap().as_str(),
            "google_books"
        );
        assert!(ProviderId::new("").is_err());
        assert!(ProviderId::new("OpenLibrary").is_err());
    }

    #[test]
    fn title_requires_main_and_deduplicates() {
        let title =
            Title::new("Sword", Some("剣".into()), vec!["Blade".into()]).expect("valid title");
        assert_eq!(title.main(), "Sword");
        assert_eq!(title.original(), Some("剣"));
        assert_eq!(title.alternatives(), &["Blade".to_string()]);
        assert_eq!(title.all().count(), 3);

        assert!(Title::new("", None, vec![]).is_err(), "blank main rejected");
        assert!(
            Title::new("Sword", None, vec!["sword".into()]).is_err(),
            "duplicate alternative rejected"
        );
        assert!(Title::new("Sword", Some("sword".into()), vec![]).is_err());
    }

    #[test]
    fn external_id_requires_value() {
        assert!(ExternalId::new(ProviderId::new("anilist").unwrap(), "42", None).is_ok());
        assert!(ExternalId::new(ProviderId::new("anilist").unwrap(), "", None).is_err());
    }
}
