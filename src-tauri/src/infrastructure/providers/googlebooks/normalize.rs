//! Pure Google Books → domain mappers (MISSION-058).
//!
//! The secondary book provider: it serves the same `Book` domain AniList...
//! rather, OpenLibrary serves, so the coordinator's parallel fan-out gives a
//! live fallback (and strong non-English + preview coverage — API_PROVIDERS §8).

use crate::domain::enums::{ContentType, MediaStatus, PersonRole};
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderPerson};
use crate::domain::value_objects::{ExternalId, ProviderId};

use super::response::{Volume, VolumeInfo};
use super::PROVIDER_ID;

/// Cap the number of categories surfaced as genres per title.
const MAX_GENRES: usize = 8;

/// Strip HTML tags and collapse whitespace (local copy shared across adapters).
fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Google Books only catalogs books.
pub(crate) fn content_type() -> ContentType {
    ContentType::Book
}

/// A known published date → completed book.
pub(crate) fn pub_status(published_date: Option<&str>) -> MediaStatus {
    match published_date {
        Some(d) if !d.starts_with("0000") => MediaStatus::Completed,
        _ => MediaStatus::Unknown,
    }
}

/// Extract the first 4-digit year from Google's free-form dates
/// (`"1965"`, `"1965-08-01"`); `"0000-00-00"` means unknown → None.
pub(crate) fn year_from_date(published_date: Option<&str>) -> Option<i32> {
    let mut buf = String::new();
    for c in published_date?.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
            if buf.len() == 4 {
                let year: i32 = buf.parse().ok()?;
                return if year == 0 { None } else { Some(year) };
            }
        } else {
            buf.clear();
        }
    }
    None
}

fn genres(info: &VolumeInfo) -> Vec<String> {
    info.categories
        .as_deref()
        .unwrap_or_default()
        .iter()
        .take(MAX_GENRES)
        .filter(|c| !c.trim().is_empty())
        .cloned()
        .collect()
}

fn cover_url(info: &VolumeInfo) -> Option<String> {
    info.image_links
        .as_ref()
        .and_then(|l| l.thumbnail.as_deref())
        .filter(|u| !u.is_empty())
        .map(ToOwned::to_owned)
}

/// A search row (or detail volume) → candidate.
pub(crate) fn candidate(volume: &Volume) -> Option<ProviderCandidate> {
    let id = volume.id.clone()?;
    let info = volume.volume_info.as_ref()?;
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: id,
        title: info.title.clone()?,
        content_type: content_type(),
        release_year: year_from_date(info.published_date.as_deref()),
        cover_url: cover_url(info),
        synopsis: None,
        external_ids: Vec::new(),
        url: info.canonical_volume_link.clone(),
    })
}

/// A volume → full `ProviderMedia`.
pub(crate) fn media(volume: &Volume) -> Option<ProviderMedia> {
    let id = volume.id.clone()?;
    let info = volume.volume_info.as_ref()?;
    let people = authors(info);
    let external_ids = external_ids(info);
    Some(ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: id,
        title_main: info.title.clone()?,
        title_original: None,
        alt_titles: info.subtitle.clone().into_iter().collect(),
        content_type: content_type(),
        format: None,
        pub_status: pub_status(info.published_date.as_deref()),
        synopsis: info
            .description
            .as_deref()
            .map(strip_html)
            .filter(|s| !s.trim().is_empty()),
        start_date: None,
        end_date: None,
        release_year: year_from_date(info.published_date.as_deref()),
        language: info.language.clone(),
        country: None,
        content_rating: None,
        pages: info.page_count.map(|n| n.max(0) as u32),
        duration_min: None,
        ep_count: None,
        ch_count: None,
        cover_url: cover_url(info),
        banner_url: None,
        url: info.canonical_volume_link.clone(),
        people,
        genres: genres(info),
        tags: Vec::new(),
        external_ids,
    })
}

/// Google Books `industryIdentifiers` → isbn10/isbn13 external ids.
pub(crate) fn external_ids(info: &VolumeInfo) -> Vec<ExternalId> {
    let mut ids = Vec::new();
    for ident in info.industry_identifiers.as_deref().unwrap_or_default() {
        let provider = match ident.r#type.as_deref() {
            Some("ISBN_10") => "isbn10",
            Some("ISBN_13") => "isbn13",
            _ => continue,
        };
        if let Ok(provider) = ProviderId::new(provider) {
            if let Ok(external) = ExternalId::new(
                provider,
                ident.identifier.as_deref().unwrap_or_default(),
                None,
            ) {
                ids.push(external);
            }
        }
    }
    ids
}

/// `authors` array → `PersonRole::Author` credits.
pub(crate) fn authors(info: &VolumeInfo) -> Vec<ProviderPerson> {
    info.authors
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|n| !n.trim().is_empty())
        .map(|name| ProviderPerson {
            role: PersonRole::Author,
            name: name.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::from_str;

    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    #[test]
    fn candidate_maps_search_row() {
        let data: super::super::response::VolumesResponse =
            from_str(&fixture("googlebooks", "search_volumes.json")).unwrap();
        let c = candidate(data.items.first().unwrap()).unwrap();
        assert_eq!(c.provider, "googlebooks");
        assert_eq!(c.provider_id, "l4YzAwAAQBAJ");
        assert_eq!(c.title, "Dune");
        assert_eq!(c.content_type, ContentType::Book);
        assert_eq!(c.release_year, Some(1965));
        assert!(c.cover_url.is_some());
        assert_eq!(
            c.url.as_deref(),
            Some("https://books.google.com/books?id=l4YzAwAAQBAJ")
        );
    }

    #[test]
    fn candidate_drops_volumes_without_id_or_title() {
        let data: super::super::response::VolumesResponse =
            from_str(&fixture("googlebooks", "search_volumes.json")).unwrap();
        let mut v = data.items[0].clone();
        v.volume_info.as_mut().unwrap().title = None;
        assert!(candidate(&v).is_none());
        v.id = None;
        assert!(candidate(&v).is_none());
    }

    #[test]
    fn year_from_date_handles_free_form_and_unknown() {
        assert_eq!(year_from_date(Some("1965")), Some(1965));
        assert_eq!(year_from_date(Some("1965-08-01")), Some(1965));
        assert_eq!(year_from_date(Some("0000-00-00")), None);
        assert_eq!(year_from_date(None), None);
    }

    #[test]
    fn media_maps_volume() {
        let data: super::super::response::VolumesResponse =
            from_str(&fixture("googlebooks", "search_volumes.json")).unwrap();
        let m = media(data.items.first().unwrap()).unwrap();
        assert_eq!(m.provider_id, "l4YzAwAAQBAJ");
        assert_eq!(m.title_main, "Dune");
        assert_eq!(m.content_type, ContentType::Book);
        assert_eq!(m.pub_status, MediaStatus::Completed);
        assert_eq!(m.release_year, Some(1965));
        assert_eq!(m.pages, Some(704));
        assert_eq!(m.language.as_deref(), Some("en"));
        assert_eq!(m.people.len(), 1);
        assert_eq!(m.people[0].name, "Frank Herbert");
        assert!(m.genres.iter().any(|g| g == "Science fiction"));
        assert!(m
            .external_ids
            .iter()
            .any(|e| e.provider().as_str() == "isbn13"));
    }
}
