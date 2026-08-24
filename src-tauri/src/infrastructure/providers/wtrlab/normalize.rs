//! WTR-LAB → domain normalization (MISSION-131).
//!
//! Pure mappers: search rows and the `__NEXT_DATA__` details payload become
//! unified provider types; the chapter feed becomes a flat chapter tree
//! (WTR-LAB novels carry no volume grouping).

use std::collections::HashSet;

use crate::domain::enums::{ContentType, MediaStatus, NodeKind, PersonRole};
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderPerson,
};

use super::response::{ChapterRow, ItemData, NextData, SearchItem};
use super::PROVIDER_ID;

/// WTR-LAB genre id taxonomy (ported from the reference lab scraper).
const GENRE_MAP: &[(i64, &str)] = &[
    (1, "Action"),
    (2, "Adult"),
    (3, "Adventure"),
    (4, "Comedy"),
    (5, "Drama"),
    (6, "Ecchi"),
    (9, "Fantasy"),
    (10, "Game"),
    (11, "Gender Bender"),
    (12, "Harem"),
    (13, "Historical"),
    (14, "Horror"),
    (15, "Josei"),
    (16, "Martial Arts"),
    (17, "Mature"),
    (18, "Mecha"),
    (19, "Military"),
    (20, "Mystery"),
    (21, "Psychological"),
    (22, "Romance"),
    (23, "School Life"),
    (24, "Sci-Fi"),
    (25, "Seinen"),
    (26, "Shoujo"),
    (27, "Shoujo Ai"),
    (28, "Shounen"),
    (29, "Shounen Ai"),
    (30, "Slice of Life"),
    (31, "Smut"),
    (32, "Sports"),
    (33, "Supernatural"),
    (34, "Tragedy"),
    (35, "Urban Life"),
    (36, "Wuxia"),
    (37, "Xianxia"),
    (38, "Xuanhuan"),
    (39, "Yaoi"),
    (40, "Yuri"),
];

fn genre_name(id: i64) -> Option<&'static str> {
    GENRE_MAP
        .iter()
        .find(|(key, _)| *key == id)
        .map(|(_, name)| *name)
}

/// `1 = completed`, anything else with a value = ongoing.
fn pub_status(status: Option<i64>) -> MediaStatus {
    match status {
        Some(1) => MediaStatus::Completed,
        Some(_) => MediaStatus::Ongoing,
        None => MediaStatus::Unknown,
    }
}

/// The composite provider id: `{id}/{slug}` — chapters need the numeric part,
/// the details page needs the slug too.
fn composite_id(id: i64, slug: &str) -> String {
    format!("{id}/{slug}")
}

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

/// Split a comma-separated author string into individual credits.
fn authors(field: Option<&str>) -> Vec<ProviderPerson> {
    field
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|name| ProviderPerson {
            role: PersonRole::Author,
            name: name.to_string(),
        })
        .collect()
}

/// A search row → candidate. Rows without a usable title drop.
pub(crate) fn candidate(item: &SearchItem) -> Option<ProviderCandidate> {
    let title = item.data.title.as_deref()?.trim();
    if title.is_empty() {
        return None;
    }
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: composite_id(item.id, &item.slug),
        title: title.to_string(),
        content_type: ContentType::WebNovel,
        release_year: None,
        cover_url: item
            .data
            .image
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        synopsis: item
            .data
            .description
            .as_deref()
            .map(strip_html)
            .filter(|s| !s.is_empty()),
        external_ids: Vec::new(),
        url: Some(format!(
            "https://wtr-lab.com/en/novel/{}",
            composite_id(item.id, &item.slug)
        )),
    })
}

/// Full details from the parsed `__NEXT_DATA__`. `None` when the serie block
/// carries no usable title.
pub(crate) fn media(next: &NextData, provider_id: &str) -> Option<ProviderMedia> {
    let serie = &next.props.page_props.serie;
    let serie_data = &serie.serie_data;
    let data: &ItemData = &serie_data.data;
    let raw_title = data.raw.title.clone();

    let title_main = data.title.clone()?;
    if title_main.trim().is_empty() {
        return None;
    }

    // Associated names (minus main/raw titles) become alt titles.
    let mut alt_titles = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for name in &serie.names {
        let candidates = name.title.iter().chain(name.raw_title.iter());
        for value in candidates {
            let value = value.trim();
            if !value.is_empty()
                && *value != title_main
                && Some(value) != raw_title.as_deref()
                && seen.insert(value.to_string())
            {
                alt_titles.push(value.to_string());
            }
        }
    }

    // Genres from the numeric taxonomy.
    let genres: Vec<String> = serie_data
        .genres
        .iter()
        .filter_map(|id| genre_name(*id).map(str::to_string))
        .collect();

    // Structured tags ride as theme tags.
    let tags: Vec<String> = next
        .props
        .page_props
        .tags
        .iter()
        .filter_map(|t| t.title.clone())
        .filter(|t| !t.is_empty())
        .collect();

    // Authors: EN field preferred, source-language names appended when they
    // differ from the romanization.
    let mut people = authors(data.author.as_deref());
    if let Some(raw_author) = data.raw.author.as_deref() {
        if !raw_author.trim().is_empty() && Some(raw_author) != data.author.as_deref() {
            for person in authors(Some(raw_author)) {
                if !people.contains(&person) {
                    people.push(person);
                }
            }
        }
    }

    Some(ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: provider_id.to_string(),
        title_main,
        title_original: raw_title,
        alt_titles,
        content_type: ContentType::WebNovel,
        format: None,
        pub_status: pub_status(serie_data.status),
        synopsis: data
            .description
            .as_deref()
            .map(strip_html)
            .filter(|s| !s.is_empty()),
        start_date: None,
        end_date: None,
        release_year: None,
        language: None,
        country: None,
        content_rating: None,
        pages: None,
        duration_min: None,
        ep_count: None,
        ch_count: serie_data
            .chapter_count
            .filter(|c| *c > 0)
            .map(|c| c as u32),
        cover_url: data.image.clone().filter(|s| !s.is_empty()),
        banner_url: None,
        url: Some(format!("https://wtr-lab.com/en/novel/{provider_id}")),
        people,
        genres,
        tags,
        external_ids: Vec::new(),
    })
}

/// Flat chapter list ordered by the feed's `order` field. Titles prefer the
/// translation, falling back to the raw name; positions are 1-based ranks.
pub(crate) fn nodes(chapters: &[ChapterRow]) -> Vec<ProviderNode> {
    let mut rows: Vec<&ChapterRow> = chapters.iter().collect();
    rows.sort_by_key(|c| c.order.unwrap_or(i64::MAX));
    rows.iter()
        .enumerate()
        .map(|(index, chapter)| {
            let position = index as i64 + 1;
            let title = chapter
                .title
                .as_deref()
                .filter(|t| !t.trim().is_empty())
                .or(chapter.name.as_deref().filter(|t| !t.trim().is_empty()))
                .map(str::to_string);
            ProviderNode {
                id: chapter
                    .order
                    .map(|o| format!("{}-ch-{}", PROVIDER_ID, o))
                    .unwrap_or_else(|| format!("{}-ch-{position}", PROVIDER_ID)),
                kind: NodeKind::Chapter,
                position,
                number: chapter.order.map(|o| o.to_string()),
                title,
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
