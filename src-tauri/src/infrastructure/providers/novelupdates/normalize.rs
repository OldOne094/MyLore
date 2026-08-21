//! NovelUpdates → domain normalization (MISSION-065).
//!
//! Pure functions mapping parsed HTML data into the unified domain types
//! (`ProviderCandidate`/`ProviderMedia`/`ProviderNode`). No I/O here, so every
//! mapping is unit-tested offline against hand-built fixtures that mirror the
//! LNReader plugin's selectors. NovelUpdates only catalogs novels, so content
//! type narrows to Novel/WebNovel from the `#showtype` text.

use crate::domain::enums::{ContentType, MediaStatus, NodeKind, PersonRole};
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderPerson,
};

use super::response::{SearchRow, SeriesPage};
use super::{page_url, PROVIDER_ID};

/// Map the `#showtype` text ("Web Novel", "Light Novel", "Published Novel")
/// to a domain content type. Anything that isn't explicitly a web novel is a
/// (printed/light) novel.
pub(crate) fn content_type(show_type: &str) -> ContentType {
    if show_type.to_lowercase().contains("web novel") {
        ContentType::WebNovel
    } else {
        ContentType::Novel
    }
}

/// Map the `#editstatus` text. NU values: Ongoing / Completed / Hiatus /
/// Cancelled / Dropped.
pub(crate) fn pub_status(status: &str) -> MediaStatus {
    let s = status.to_lowercase();
    if s.contains("ongoing") {
        MediaStatus::Ongoing
    } else if s.contains("hiatus") {
        MediaStatus::Hiatus
    } else if s.contains("cancelled") || s.contains("dropped") {
        MediaStatus::Cancelled
    } else if s.contains("completed") {
        MediaStatus::Completed
    } else {
        MediaStatus::Unknown
    }
}

/// A search row → candidate. `None` when the row lacks a resolvable slug/title.
pub(crate) fn candidate(row: &SearchRow) -> Option<ProviderCandidate> {
    if row.slug.is_empty() || row.title.is_empty() {
        return None;
    }
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: row.slug.clone(),
        title: row.title.clone(),
        content_type: ContentType::Novel, // search rows don't carry #showtype
        release_year: None,
        cover_url: row.cover.clone(),
        synopsis: None,
        external_ids: Vec::new(),
        url: Some(page_url(&row.slug)),
    })
}

/// A parsed series page → full `ProviderMedia`. `provider_id` is the caller's
/// slug (the page itself doesn't carry it). `ch_count` is deliberately `None`
/// (the chapter count lives in the feed, a separate call).
pub(crate) fn media(page: &SeriesPage, provider_id: &str) -> ProviderMedia {
    ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: provider_id.to_string(),
        title_main: page.title.clone(),
        title_original: None,
        alt_titles: Vec::new(),
        content_type: content_type(&page.show_type),
        format: Some(page.show_type.clone()),
        pub_status: pub_status(&page.status),
        synopsis: page.synopsis.clone(),
        start_date: None,
        end_date: None,
        release_year: None,
        language: None,
        country: None,
        content_rating: None,
        pages: None,
        duration_min: None,
        ep_count: None,
        ch_count: None,
        cover_url: page.cover.clone(),
        banner_url: None,
        url: Some(page_url(provider_id)),
        people: authors(&page.authors),
        genres: page.genres.clone(),
        tags: Vec::new(),
        external_ids: Vec::new(),
    }
}

fn authors(names: &[String]) -> Vec<ProviderPerson> {
    names
        .iter()
        .filter(|n| !n.trim().is_empty())
        .map(|name| ProviderPerson {
            role: PersonRole::Author,
            name: name.clone(),
        })
        .collect()
}

/// A parsed NU chapter label like `v1c1part1`, `c3`, `ss1`, `s12`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterLabel {
    pub volume: Option<u32>,
    pub chapter: Option<u32>,
    pub part: Option<u32>,
    /// Side stories / specials (`ss{n}`, `s{n}`).
    pub is_special: bool,
    /// The raw label as served (kept for human titles).
    pub raw: String,
}

/// Parse an NU chapter label. NU labels are compact: `v`olume, `c`hapter,
/// `part`, `ss`/`s` (side story). Anything not matching those markers is
/// treated as a special.
pub(crate) fn parse_label(raw: &str) -> ChapterLabel {
    let lower = raw.trim().to_lowercase();
    let mut label = ChapterLabel {
        volume: None,
        chapter: None,
        part: None,
        is_special: false,
        raw: raw.trim().to_string(),
    };

    let bytes = lower.as_bytes();
    let mut i = 0;

    if bytes.starts_with(b"v") {
        let (num, next) = take_digits(bytes, 1);
        if let Some(n) = num {
            label.volume = Some(n);
            i = next;
        }
    }
    if bytes.get(i) == Some(&b'c') {
        let (num, next) = take_digits(bytes, i + 1);
        if let Some(n) = num {
            label.chapter = Some(n);
            i = next;
        }
    }
    if bytes[i..].starts_with(b"part") {
        let (num, _) = take_digits(bytes, i + 4);
        if let Some(n) = num {
            label.part = Some(n);
        }
    }

    // No volume/chapter/part matched → likely a side story or unparseable label.
    if label.volume.is_none() && label.chapter.is_none() {
        label.is_special = true;
        if bytes.starts_with(b"ss") || bytes.starts_with(b"s") {
            label.chapter = take_digits(bytes, if bytes.starts_with(b"ss") { 2 } else { 1 }).0;
        }
    }
    label
}

/// Read a run of digits starting at `start`; returns `(number, index_after)`.
fn take_digits(bytes: &[u8], start: usize) -> (Option<u32>, usize) {
    let mut i = start;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == start {
        return (None, i);
    }
    let num = std::str::from_utf8(&bytes[start..i])
        .ok()
        .and_then(|s| s.parse().ok());
    (num, i)
}

/// Human title for a label: `v1c1part1` → "Volume 1 Chapter 1 Part 1".
fn chapter_title(label: &ChapterLabel) -> String {
    let mut parts = Vec::new();
    if let Some(v) = label.volume {
        parts.push(format!("Volume {v}"));
    }
    if let Some(c) = label.chapter {
        parts.push(if label.is_special {
            format!("Special {c}")
        } else {
            format!("Chapter {c}")
        });
    }
    if let Some(p) = label.part {
        parts.push(format!("Part {p}"));
    }
    if parts.is_empty() {
        label.raw.clone()
    } else {
        parts.join(" ")
    }
}

/// Build the volume/chapter node tree from the chapter feed (served
/// newest-first, so reversed to chronological). Chapters with a volume group
/// under a `Volume` parent; volume-less/special chapters sit at the top level.
pub(crate) fn nodes(labels: &[String], provider_id: &str) -> Vec<ProviderNode> {
    let mut volumes: Vec<ProviderNode> = Vec::new();
    let mut loose: Vec<ProviderNode> = Vec::new();
    for (idx, raw) in labels.iter().rev().enumerate() {
        let label = parse_label(raw);
        let node = chapter_node(&label, provider_id, idx as i64 + 1);
        match label.volume {
            Some(volume) => push_into_volume(&mut volumes, provider_id, volume, node),
            None => loose.push(node),
        }
    }
    volumes.extend(loose);
    volumes
}

fn chapter_node(label: &ChapterLabel, provider_id: &str, position: i64) -> ProviderNode {
    let number = label
        .chapter
        .map(|c| c.to_string())
        .or_else(|| label.raw.clone().into());
    ProviderNode {
        // NU chapter labels aren't globally unique → synthesize a stable id.
        id: format!("{provider_id}-ch-{position}"),
        kind: NodeKind::Chapter,
        position,
        number,
        title: Some(chapter_title(label)),
        release_date: None,
        duration_min: None,
        page_count: None,
        synopsis: None,
        is_special: label.is_special,
        children: Vec::new(),
    }
}

fn push_into_volume(
    volumes: &mut Vec<ProviderNode>,
    provider_id: &str,
    volume: u32,
    chapter: ProviderNode,
) {
    let key = volume.to_string();
    match volumes.last_mut() {
        Some(last) if last.number.as_deref() == Some(&key) => last.children.push(chapter),
        _ => volumes.push(ProviderNode {
            id: format!("{provider_id}-vol-{volume}"),
            kind: NodeKind::Volume,
            position: volume as i64,
            number: Some(key),
            title: None,
            release_date: None,
            duration_min: None,
            page_count: None,
            synopsis: None,
            is_special: false,
            children: vec![chapter],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::providers::test_support::novelupdates_fixture;

    fn search_row() -> SearchRow {
        let page =
            super::super::response::parse_search_rows(&novelupdates_fixture("search_series.html"));
        page.into_iter()
            .find(|r| r.slug == "dungeon-defender")
            .unwrap()
    }

    fn series_page() -> SeriesPage {
        super::super::response::parse_series_page(&novelupdates_fixture(
            "series_dungeon_defender.html",
        ))
        .expect("series page present")
    }

    #[test]
    fn content_type_maps_showtype() {
        assert_eq!(content_type("Web Novel"), ContentType::WebNovel);
        assert_eq!(content_type("Light Novel"), ContentType::Novel);
        assert_eq!(content_type("Published Novel"), ContentType::Novel);
        assert_eq!(content_type(""), ContentType::Novel);
    }

    #[test]
    fn pub_status_maps_nu_values() {
        assert_eq!(pub_status("Ongoing"), MediaStatus::Ongoing);
        assert_eq!(pub_status("Completed"), MediaStatus::Completed);
        assert_eq!(pub_status("Hiatus"), MediaStatus::Hiatus);
        assert_eq!(pub_status("Cancelled"), MediaStatus::Cancelled);
        assert_eq!(pub_status("Dropped"), MediaStatus::Cancelled);
        assert_eq!(pub_status("Unknown"), MediaStatus::Unknown);
    }

    #[test]
    fn candidate_maps_search_row() {
        let c = candidate(&search_row()).unwrap();
        assert_eq!(c.provider, "novelupdates");
        assert_eq!(c.provider_id, "dungeon-defender");
        assert_eq!(c.title, "Dungeon Defender");
        assert_eq!(c.content_type, ContentType::Novel);
        assert!(c
            .cover_url
            .as_deref()
            .unwrap()
            .contains("cdn.novelupdates.com"));
        assert_eq!(
            c.url.as_deref(),
            Some("https://www.novelupdates.com/series/dungeon-defender/")
        );
    }

    #[test]
    fn candidate_drops_empty_slugs() {
        assert!(candidate(&SearchRow {
            title: "X".into(),
            slug: String::new(),
            cover: None
        })
        .is_none());
    }

    #[test]
    fn media_maps_series_page() {
        let m = media(&series_page(), "dungeon-defender");
        assert_eq!(m.provider_id, "dungeon-defender");
        assert_eq!(m.title_main, "Dungeon Defender");
        assert_eq!(m.content_type, ContentType::WebNovel);
        assert_eq!(m.format.as_deref(), Some("Web Novel"));
        assert_eq!(m.pub_status, MediaStatus::Ongoing);
        assert!(m
            .people
            .iter()
            .any(|p| p.role == PersonRole::Author && p.name == "Golam"));
        assert!(m.genres.iter().any(|g| g == "Action"));
        assert!(m.synopsis.as_deref().unwrap().contains("Lester"));
        assert!(m
            .cover_url
            .as_deref()
            .unwrap()
            .contains("cdn.novelupdates.com"));
    }

    #[test]
    fn label_parsing_handles_nu_formats() {
        let l = parse_label("v1c1part1");
        assert_eq!(l.volume, Some(1));
        assert_eq!(l.chapter, Some(1));
        assert_eq!(l.part, Some(1));
        assert!(!l.is_special);

        let l = parse_label("c3");
        assert_eq!(l.volume, None);
        assert_eq!(l.chapter, Some(3));
        assert_eq!(l.part, None);

        let l = parse_label("ss1");
        assert!(l.is_special);
        assert_eq!(l.chapter, Some(1));

        let l = parse_label("s12");
        assert!(l.is_special);
        assert_eq!(l.chapter, Some(12));

        let l = parse_label("v2c4");
        assert_eq!(l.volume, Some(2));
        assert_eq!(l.chapter, Some(4));
    }

    #[test]
    fn chapter_title_humanizes() {
        assert_eq!(
            chapter_title(&parse_label("v1c1part1")),
            "Volume 1 Chapter 1 Part 1"
        );
        assert_eq!(chapter_title(&parse_label("c3")), "Chapter 3");
        assert_eq!(chapter_title(&parse_label("ss1")), "Special 1");
    }

    #[test]
    fn feed_builds_volume_and_loose_chapters_chronological() {
        let page = super::super::response::parse_chapter_labels(&novelupdates_fixture(
            "chapters_dungeon_defender.html",
        ));
        let tree = nodes(&page, "dungeon-defender");
        assert_eq!(tree.len(), 2, "volume 1 + loose special");
        let vol1 = &tree[0];
        assert_eq!(vol1.kind, NodeKind::Volume);
        assert_eq!(vol1.number.as_deref(), Some("1"));
        assert_eq!(vol1.id, "dungeon-defender-vol-1");
        assert_eq!(vol1.children.len(), 3);
        assert_eq!(
            vol1.children[0].title.as_deref(),
            Some("Volume 1 Chapter 4 Part 1"),
            "newest-first feed reversed to chronological"
        );
        assert_eq!(
            vol1.children[2].title.as_deref(),
            Some("Volume 1 Chapter 4 Part 3")
        );
        assert_eq!(
            tree[1].kind,
            NodeKind::Chapter,
            "volume-less special is top-level"
        );
        assert!(tree[1].is_special);
    }
}
