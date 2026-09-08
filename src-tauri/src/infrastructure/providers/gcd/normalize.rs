//! GCD → domain normalization (MISSION-109omics).

use crate::domain::enums::{ContentType, MediaStatus, NodeKind};
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderNode};

use super::response::Series;
use super::PROVIDER_ID;

fn year_from(series: &Series) -> Option<i32> {
    series.year_began.map(|y| y as i32)
}

fn pub_status(series: &Series) -> MediaStatus {
    match (series.year_began, series.year_ended) {
        (Some(_), Some(_)) => MediaStatus::Completed,
        (Some(_), None) => MediaStatus::Ongoing,
        _ => MediaStatus::Unknown,
    }
}

fn series_id(api_url: Option<&str>) -> String {
    // api_url like https://www.comics.org/api/series/10814/?format=json
    api_url
        .and_then(|u| {
            u.trim_end_matches('/')
                .trim_end_matches("?format=json")
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn candidate(series: &Series) -> Option<ProviderCandidate> {
    let title = series.name.as_deref()?.trim();
    if title.is_empty() {
        return None;
    }
    let id = series_id(series.api_url.as_deref());
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: id.clone(),
        title: title.to_string(),
        content_type: ContentType::Comic,
        release_year: year_from(series),
        cover_url: None,
        synopsis: None,
        external_ids: Vec::new(),
        url: Some(format!("https://www.comics.org/series/{id}/")),
    })
}

pub(crate) fn media(series: &Series, provider_id: &str) -> Option<ProviderMedia> {
    let title_main = series.name.clone()?.trim().to_string();
    if title_main.is_empty() {
        return None;
    }
    Some(ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: provider_id.to_string(),
        title_main,
        title_original: None,
        alt_titles: Vec::new(),
        content_type: ContentType::Comic,
        format: None,
        pub_status: pub_status(series),
        synopsis: None,
        start_date: None,
        end_date: None,
        release_year: year_from(series),
        language: series.language.clone(),
        country: series.country.clone(),
        content_rating: None,
        pages: None,
        duration_min: None,
        ep_count: None,
        ch_count: series
            .active_issues
            .as_ref()
            .map(|v| v.len() as u32)
            .filter(|c| *c > 0),
        cover_url: None,
        banner_url: None,
        url: Some(format!("https://www.comics.org/series/{provider_id}/")),
        people: Vec::new(),
        genres: Vec::new(),
        tags: Vec::new(),
        external_ids: Vec::new(),
    })
}

pub(crate) fn nodes(series: &Series) -> Vec<ProviderNode> {
    let descriptors = series.issue_descriptors.as_deref().unwrap_or_default();
    descriptors
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            let position = index as i64 + 1;
            ProviderNode {
                id: format!("{PROVIDER_ID}-issue-{position}"),
                kind: NodeKind::Chapter,
                position,
                number: Some(descriptor.clone()),
                title: None,
                release_date: None,
                duration_min: None,
                page_count: None,
                synopsis: None,
                is_special: false,
                children: Vec::new(),
            }
        })
        .collect()
}
