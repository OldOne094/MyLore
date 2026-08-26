//! RAWG → domain normalization (MISSION-109).

use crate::domain::enums::{ContentType, MediaStatus};
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia};

use super::response::{GameDetail, SearchItem};
use super::PROVIDER_ID;

/// `released` is ISO date; extract the year prefix.
fn year_from(released: Option<&str>) -> Option<i32> {
    released?.get(..4)?.parse().ok()
}

/// A released date implies completed; no date = unknown.
fn pub_status(released: Option<&str>) -> MediaStatus {
    match released {
        Some(d) if !d.trim().is_empty() => MediaStatus::Completed,
        _ => MediaStatus::Unknown,
    }
}

pub(crate) fn candidate(item: &SearchItem) -> Option<ProviderCandidate> {
    let title = item.name.as_deref()?.trim();
    if title.is_empty() {
        return None;
    }
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: item.id.to_string(),
        title: title.to_string(),
        content_type: ContentType::Game,
        release_year: year_from(item.released.as_deref()),
        cover_url: item
            .background_image
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        synopsis: None,
        external_ids: Vec::new(),
        url: Some(format!("https://rawg.io/games/{}", item.id)),
    })
}

pub(crate) fn media(detail: &GameDetail) -> Option<ProviderMedia> {
    let title_main = detail.name.clone()?;
    if title_main.trim().is_empty() {
        return None;
    }

    let genres: Vec<String> = detail
        .genres
        .iter()
        .filter_map(|g| g.name.clone())
        .collect();

    Some(ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: detail.id.to_string(),
        title_main,
        title_original: detail.name_original.clone(),
        alt_titles: Vec::new(),
        content_type: ContentType::Game,
        format: None,
        pub_status: pub_status(detail.released.as_deref()),
        synopsis: detail
            .description_raw
            .as_deref()
            .map(str::to_string)
            .filter(|s| !s.trim().is_empty()),
        start_date: None,
        end_date: None,
        release_year: year_from(detail.released.as_deref()),
        language: None,
        country: None,
        content_rating: None,
        pages: None,
        duration_min: None,
        ep_count: None,
        ch_count: None,
        cover_url: detail.background_image.clone().filter(|s| !s.is_empty()),
        banner_url: None,
        url: Some(format!("https://rawg.io/games/{}", detail.id)),
        people: Vec::new(),
        genres,
        tags: Vec::new(),
        external_ids: Vec::new(),
    })
}
