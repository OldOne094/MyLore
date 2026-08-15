//! Pure Jikan → domain mappers (MISSION-058).
//!
//! Jikan mirrors MAL data, so the MAL id *is* the provider id; the only
//! cross-provider id we surface is `mal` (used for dedup regardless of which
//! provider a title originally came from — API_PROVIDERS §2).

use crate::domain::enums::{ContentType, MediaStatus, NodeKind, PersonRole};
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderPerson,
};
use crate::domain::value_objects::{ExternalId, ProviderId};

use super::response::{Anime, Episode};
use super::PROVIDER_ID;

/// `"TV"`/`"Movie"` etc. map like AniList's `format`: anime shows stay Anime,
/// anime films become Movie.
pub(crate) fn content_type(media_type: Option<&str>) -> ContentType {
    match media_type {
        Some("Movie") => ContentType::Movie,
        _ => ContentType::Anime,
    }
}

/// MAL status strings → domain status.
pub(crate) fn pub_status(status: Option<&str>) -> MediaStatus {
    match status {
        Some("Finished Airing") => MediaStatus::Completed,
        Some("Currently Airing") => MediaStatus::Ongoing,
        Some("Not yet aired") => MediaStatus::Announced,
        _ => MediaStatus::Unknown,
    }
}

/// `"24 min per ep"` → 24 (first integer run).
fn duration_min(duration: Option<&str>) -> Option<u32> {
    let mut buf = String::new();
    for c in duration?.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
        } else if !buf.is_empty() {
            return buf.parse().ok();
        }
    }
    buf.parse().ok()
}

/// ISO date `"2009-04-05T00:00:00+00:00"` → `"2009-04-05"`.
fn date_prefix(date: Option<&str>) -> Option<String> {
    date.map(|d| d.chars().take(10).collect())
}

/// MAL CDN cover; prefer the large variant.
fn cover_url(anime: &Anime) -> Option<String> {
    let jpg = anime.images.as_ref()?.jpg.as_ref()?;
    jpg.large_image_url
        .as_deref()
        .or(jpg.image_url.as_deref())
        .filter(|u| !u.is_empty())
        .map(ToOwned::to_owned)
}

/// A search row → candidate.
pub(crate) fn candidate(anime: &Anime) -> Option<ProviderCandidate> {
    let id = anime.mal_id?.to_string();
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: id,
        title: anime.title.clone()?,
        content_type: content_type(anime.r#type.as_deref()),
        release_year: anime.year.map(|y| y as i32),
        cover_url: cover_url(anime),
        synopsis: None,
        external_ids: Vec::new(),
        url: anime.url.clone(),
    })
}

/// Author credits don't exist on MAL (studios instead).
pub(crate) fn people(anime: &Anime) -> Vec<ProviderPerson> {
    anime
        .studios
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.name.as_deref())
        .filter(|n| !n.trim().is_empty())
        .map(|name| ProviderPerson {
            role: PersonRole::Studio,
            name: name.to_string(),
        })
        .collect()
}

fn names(list: Option<&Vec<super::response::Genre>>) -> Vec<String> {
    list.map(|l| l.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|g| g.name.clone())
        .filter(|n| !n.trim().is_empty())
        .collect()
}

/// Full details → `ProviderMedia`. `external_ids` carries the MAL id itself.
pub(crate) fn media(anime: &Anime) -> Option<ProviderMedia> {
    let id = anime.mal_id?.to_string();
    let external_ids = push_mal(vec![], &id);
    Some(ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: id.clone(),
        title_main: anime.title.clone()?,
        title_original: anime.title_japanese.clone(),
        alt_titles: Vec::new(),
        content_type: content_type(anime.r#type.as_deref()),
        format: anime.r#type.clone(),
        pub_status: pub_status(anime.status.as_deref()),
        synopsis: anime
            .synopsis
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned),
        start_date: date_prefix(anime.aired.as_ref().and_then(|a| a.from.as_deref())),
        end_date: date_prefix(anime.aired.as_ref().and_then(|a| a.to.as_deref())),
        release_year: anime.year.map(|y| y as i32),
        language: None,
        country: None,
        content_rating: anime.rating.clone(),
        pages: None,
        duration_min: duration_min(anime.duration.as_deref()),
        ep_count: anime.episodes.map(|e| e as u32),
        ch_count: None,
        cover_url: cover_url(anime),
        banner_url: None,
        url: anime.url.clone(),
        people: people(anime),
        genres: names(anime.genres.as_ref()),
        tags: names(anime.themes.as_ref()),
        external_ids,
    })
}

/// Episodes → a flat Episode node list (no volumes/parts on MAL). Jikan does
/// not return an explicit episode number, so the list ordinal is the number.
pub(crate) fn nodes(episodes: &[Episode], provider_id: &str) -> Vec<ProviderNode> {
    episodes
        .iter()
        .enumerate()
        .map(|(idx, ep)| ProviderNode {
            id: format!("{provider_id}-e{}", idx + 1),
            kind: NodeKind::Episode,
            position: idx as i64 + 1,
            number: Some((idx + 1).to_string()),
            title: ep.title.clone().or_else(|| ep.title_romanji.clone()),
            release_date: None, // free-form "Apr 5, 2009", not ISO
            duration_min: None,
            page_count: None,
            synopsis: None,
            is_special: false,
            children: Vec::new(),
        })
        .collect()
}

/// The MAL id is surfaced as an external id for dedup (MAL-native ids must
/// always be stored regardless of the provider that served the title).
pub(crate) fn push_mal(mut out: Vec<ExternalId>, provider_id: &str) -> Vec<ExternalId> {
    if let Ok(provider) = ProviderId::new("mal") {
        if let Ok(id) = ExternalId::new(provider, provider_id, None) {
            out.push(id);
        }
    }
    out
}
