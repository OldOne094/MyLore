//! MangaDex → domain normalization (MISSION-056).
//!
//! Pure functions mapping the serde response models into the unified domain
//! types (`ProviderCandidate`/`ProviderMedia`/`ProviderNode`/`ExternalId`). No
//! I/O here, so every mapping is unit-tested offline against recorded fixtures
//! (`tests/fixtures/mangadex/`). MangaDex ids are UUIDs (unique across kinds),
//! so provider ids carry no kind prefix.

use std::collections::{HashMap, HashSet};

use crate::domain::enums::{ContentType, MediaStatus, NodeKind, PersonRole};
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderPerson,
};
use crate::domain::value_objects::{ExternalId, ProviderId};

use super::response::{Chapter, Manga, MangaAttributes, Relationship, Tag};
use super::PROVIDER_ID;

/// Covers are served from the MangaDex CDN at `.256.jpg` (list) and `.512.jpg`
/// (detail); `.256.jpg` is a good balance for both.
pub(crate) const COVER_SIZE: &str = "256";
const COVER_BASE: &str = "https://uploads.mangadex.org/covers";

/// Map MangaDex's `format` to a domain content type. `one_shot`/`doujinshi`
/// are still manga; manhwa/manhua are explicit formats; `novel` covers light
/// and web novels hosted on MangaDex.
pub(crate) fn content_type(format: Option<&str>) -> ContentType {
    match format {
        Some("manga") | Some("one_shot") | Some("doujinshi") => ContentType::Manga,
        Some("manhwa") => ContentType::Manhwa,
        Some("manhua") => ContentType::Manhua,
        Some("novel") => ContentType::Novel,
        _ => ContentType::Other,
    }
}

pub(crate) fn pub_status(status: Option<&str>) -> MediaStatus {
    match status {
        Some("ongoing") => MediaStatus::Ongoing,
        Some("completed") | Some("published") => MediaStatus::Completed,
        Some("hiatus") => MediaStatus::Hiatus,
        Some("cancelled") => MediaStatus::Cancelled,
        _ => MediaStatus::Unknown,
    }
}

/// The best localized value for a locale→text map: first preferred language,
/// then the original language, then any non-blank value.
fn localized(
    map: &HashMap<String, String>,
    preferred: &[&str],
    original_language: Option<&str>,
) -> Option<String> {
    for lang in preferred {
        if let Some(v) = map.get(*lang).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            return Some(v.to_string());
        }
    }
    if let Some(lang) = original_language {
        if let Some(v) = map.get(lang).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            return Some(v.to_string());
        }
    }
    map.values()
        .find(|v| !v.trim().is_empty())
        .map(|v| v.trim().to_string())
}

fn main_title(attrs: &MangaAttributes) -> String {
    localized(&attrs.title, &["en"], attrs.original_language.as_deref()).unwrap_or_default()
}

fn original_title(attrs: &MangaAttributes) -> Option<String> {
    let main = main_title(attrs);
    attrs
        .original_language
        .as_deref()
        .and_then(|lang| attrs.title.get(lang).map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty() && *s != main)
}

fn alt_titles(attrs: &MangaAttributes) -> Vec<String> {
    let main = main_title(attrs);
    let original = original_title(attrs);
    let mut alts = Vec::new();
    for entry in &attrs.alt_titles {
        for value in entry.values() {
            let value = value.trim().to_string();
            if !value.is_empty()
                && value != main
                && Some(&value) != original.as_ref()
                && !alts.contains(&value)
            {
                alts.push(value);
            }
        }
    }
    alts
}

fn synopsis(attrs: &MangaAttributes) -> Option<String> {
    let raw = localized(
        &attrs.description,
        &["en"],
        attrs.original_language.as_deref(),
    )
    .unwrap_or_default();
    let clean = strip_html(&raw);
    let clean = clean.trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean.to_string())
    }
}

/// Strip HTML tags and collapse whitespace (MangaDex descriptions may carry
/// `<br>` and other markup). Shared with the AniList adapter — one sanitizer
/// for all adapters.
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

fn cover_url(manga_id: &str, relationships: &[Relationship]) -> Option<String> {
    let file = relationships
        .iter()
        .find(|r| r.kind.as_deref() == Some("cover_art"))
        .and_then(|r| r.attributes.as_ref())
        .and_then(|a| a.file_name.as_deref())
        .filter(|f| !f.is_empty());
    file.map(|f| format!("{COVER_BASE}/{manga_id}/{f}.{COVER_SIZE}.jpg"))
}

/// Map a search row (a `Manga`) to a candidate. Returns `None` when the
/// format maps to `Other`.
pub(crate) fn candidate(manga: &Manga) -> Option<ProviderCandidate> {
    let content_type = content_type(manga.attributes.format.as_deref());
    if content_type == ContentType::Other {
        return None;
    }
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: manga.id.clone(),
        title: main_title(&manga.attributes),
        content_type,
        release_year: manga.attributes.year.map(|y| y as i32),
        cover_url: cover_url(&manga.id, &manga.relationships),
        synopsis: synopsis(&manga.attributes),
        external_ids: Vec::new(),
        url: Some(format!("https://mangadex.org/title/{}", manga.id)),
    })
}

/// Map full manga details to `ProviderMedia`. `ch_count` is deliberately
/// `None` (the `/manga` payload has no chapter count; the feed is a separate
/// call).
pub(crate) fn media(manga: &Manga) -> ProviderMedia {
    let attrs = &manga.attributes;
    ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: manga.id.clone(),
        title_main: main_title(attrs),
        title_original: original_title(attrs),
        alt_titles: alt_titles(attrs),
        content_type: content_type(attrs.format.as_deref()),
        format: attrs.format.clone(),
        pub_status: pub_status(attrs.status.as_deref()),
        synopsis: synopsis(attrs),
        start_date: None,
        end_date: None,
        release_year: attrs.year.map(|y| y as i32),
        language: attrs.original_language.clone(),
        country: None,
        content_rating: attrs.content_rating.clone(),
        pages: None,
        duration_min: None,
        ep_count: None,
        ch_count: None,
        cover_url: cover_url(&manga.id, &manga.relationships),
        banner_url: None,
        url: Some(format!("https://mangadex.org/title/{}", manga.id)),
        people: people(&manga.relationships),
        genres: tag_names(&attrs.tags, "genre"),
        tags: tag_names(&attrs.tags, "theme"),
        external_ids: external_ids(attrs.links.as_ref()),
    }
}

/// Authors and artists from relationships (deduped per `(role, name)`).
pub(crate) fn people(relationships: &[Relationship]) -> Vec<ProviderPerson> {
    let mut out = Vec::new();
    let mut seen: HashSet<(PersonRole, String)> = HashSet::new();
    for relationship in relationships {
        let Some(name) = relationship
            .attributes
            .as_ref()
            .and_then(|a| a.name.as_deref())
        else {
            continue;
        };
        let role = match relationship.kind.as_deref() {
            Some("author") => PersonRole::Author,
            Some("artist") => PersonRole::Artist,
            _ => continue,
        };
        let name = name.trim();
        if name.is_empty() || !seen.insert((role, name.to_string())) {
            continue;
        }
        out.push(ProviderPerson {
            role,
            name: name.to_string(),
        });
    }
    out
}

fn tag_names(tags: &[Tag], group: &str) -> Vec<String> {
    tags.iter()
        .filter(|t| t.attributes.group.as_deref() == Some(group))
        .filter_map(|t| localized(&t.attributes.name, &["en"], None))
        .collect()
}

/// Build the chapter/volume node tree from a feed (already ordered by volume
/// asc, chapter asc). Chapters with a `volume` group under a `Volume` parent;
/// volume-less chapters sit at the top level.
pub(crate) fn nodes(chapters: &[Chapter], manga_id: &str) -> Vec<ProviderNode> {
    let mut volumes: Vec<ProviderNode> = Vec::new();
    let mut loose: Vec<ProviderNode> = Vec::new();
    for (idx, chapter) in chapters.iter().enumerate() {
        let node = chapter_node(chapter, idx as i64 + 1);
        match chapter
            .attributes
            .volume
            .as_deref()
            .filter(|v| !v.is_empty())
        {
            Some(volume) => push_into_volume(&mut volumes, manga_id, volume, node),
            None => loose.push(node),
        }
    }
    volumes.extend(loose);
    volumes
}

fn chapter_node(chapter: &Chapter, position: i64) -> ProviderNode {
    ProviderNode {
        // MangaDex chapter ids are UUIDs, unique across the site.
        id: chapter.id.clone(),
        kind: NodeKind::Chapter,
        position,
        number: chapter
            .attributes
            .chapter
            .clone()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty()),
        title: chapter
            .attributes
            .title
            .clone()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty()),
        release_date: chapter
            .attributes
            .publish_at
            .as_deref()
            .and_then(|p| p.get(..10))
            .map(|d| d.to_string()),
        duration_min: None,
        page_count: chapter.attributes.pages,
        synopsis: None,
        is_special: false,
        children: Vec::new(),
    }
}

fn push_into_volume(
    volumes: &mut Vec<ProviderNode>,
    manga_id: &str,
    volume: &str,
    chapter: ProviderNode,
) {
    let key = volume.to_string();
    match volumes.last_mut() {
        Some(last) if last.number.as_deref() == Some(&key) => last.children.push(chapter),
        _ => volumes.push(ProviderNode {
            id: format!("{manga_id}-vol-{volume}"),
            kind: NodeKind::Volume,
            position: volume
                .split('.')
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| volumes.len() as i64 + 1),
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

/// Cross-provider ids from `links`. Canonical provider ids mirror the AniList
/// adapter's `site_to_provider`; known sites get a canonical URL.
pub(crate) fn external_ids(links: Option<&HashMap<String, String>>) -> Vec<ExternalId> {
    let mut out = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let Some(links) = links else { return out };
    for (site, value) in links {
        let Some(provider) = site_to_provider(site) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || !seen.insert((provider.to_string(), value.to_string())) {
            continue;
        }
        push_external(&mut out, provider, value, link_url(provider, value));
    }
    out
}

fn site_to_provider(site: &str) -> Option<&'static str> {
    match site {
        "mal" => Some("mal"),
        "anilist" => Some("anilist"),
        "kt" => Some("kitsu"),
        "ap" => Some("anime_planet"),
        "bw" => Some("bookwalker"),
        "nu" => Some("novelupdates"),
        "mu" => Some("mangaupdates"),
        "amz" => Some("amazon"),
        _ => None,
    }
}

fn link_url(provider: &str, value: &str) -> Option<String> {
    match provider {
        "mal" => Some(format!("https://myanimelist.net/manga/{value}/")),
        "anilist" => Some(format!("https://anilist.co/manga/{value}/")),
        "kitsu" => Some(format!("https://kitsu.app/manga/{value}")),
        _ => None,
    }
}

fn push_external(out: &mut Vec<ExternalId>, provider: &str, value: &str, url: Option<String>) {
    if let Ok(provider) = ProviderId::new(provider) {
        if let Ok(external) = ExternalId::new(provider, value, url) {
            out.push(external);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
        serde_json::from_str(&fixture("mangadex", name)).expect("fixture parses")
    }

    fn berserk() -> Manga {
        let data: super::super::response::MangaSingleResponse = parse("details_manga.json");
        data.data.expect("manga present")
    }

    #[test]
    fn content_type_maps_formats() {
        assert_eq!(content_type(Some("manga")), ContentType::Manga);
        assert_eq!(content_type(Some("one_shot")), ContentType::Manga);
        assert_eq!(content_type(Some("doujinshi")), ContentType::Manga);
        assert_eq!(content_type(Some("manhwa")), ContentType::Manhwa);
        assert_eq!(content_type(Some("manhua")), ContentType::Manhua);
        assert_eq!(content_type(Some("novel")), ContentType::Novel);
        assert_eq!(content_type(Some("anime")), ContentType::Other);
        assert_eq!(content_type(None), ContentType::Other);
    }

    #[test]
    fn pub_status_maps_mangadex_values() {
        assert_eq!(pub_status(Some("ongoing")), MediaStatus::Ongoing);
        assert_eq!(pub_status(Some("completed")), MediaStatus::Completed);
        assert_eq!(pub_status(Some("published")), MediaStatus::Completed);
        assert_eq!(pub_status(Some("hiatus")), MediaStatus::Hiatus);
        assert_eq!(pub_status(Some("cancelled")), MediaStatus::Cancelled);
        assert_eq!(pub_status(Some("nope")), MediaStatus::Unknown);
    }

    #[test]
    fn search_rows_map_to_candidates() {
        let data: super::super::response::MangaListResponse = parse("search_manga.json");
        let hits: Vec<ProviderCandidate> = data.data.iter().filter_map(candidate).collect();
        assert_eq!(hits.len(), 3, "manga, manhwa, novel");
        let berserk = hits
            .iter()
            .find(|h| h.provider_id.starts_with("1111"))
            .unwrap();
        assert_eq!(berserk.title, "Berserk");
        assert_eq!(berserk.content_type, ContentType::Manga);
        assert_eq!(berserk.release_year, Some(1989));
        assert_eq!(
            berserk.url.as_deref(),
            Some("https://mangadex.org/title/11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(berserk.cover_url.as_deref(),
            Some("https://uploads.mangadex.org/covers/11111111-1111-1111-1111-111111111111/f1.256.jpg"));
        assert!(berserk.synopsis.as_deref().unwrap().contains("Guts"));
        let solo = hits
            .iter()
            .find(|h| h.provider_id.starts_with("2222"))
            .unwrap();
        assert_eq!(solo.content_type, ContentType::Manhwa);
        assert_eq!(solo.title, "Solo Leveling");
        let overlord = hits
            .iter()
            .find(|h| h.provider_id.starts_with("3333"))
            .unwrap();
        assert_eq!(overlord.content_type, ContentType::Novel);
    }

    #[test]
    fn details_normalize_fully() {
        let m = media(&berserk());
        assert_eq!(m.provider_id, "11111111-1111-1111-1111-111111111111");
        assert_eq!(m.title_main, "Berserk");
        assert_eq!(m.title_original.as_deref(), Some("ベルセルク"));
        assert!(m
            .alt_titles
            .contains(&"Berserk (Miura, Kentarou)".to_string()));
        assert_eq!(m.content_type, ContentType::Manga);
        assert_eq!(m.format.as_deref(), Some("manga"));
        assert_eq!(m.pub_status, MediaStatus::Ongoing);
        assert_eq!(m.release_year, Some(1989));
        assert_eq!(m.language.as_deref(), Some("ja"));
        assert_eq!(m.content_rating.as_deref(), Some("safe"));
        assert!(m.synopsis.as_deref().unwrap().contains("mercenary Guts"));
        assert_eq!(m.genres, vec!["Action", "Dark Fantasy"]);
        assert_eq!(m.tags, vec!["Survival"]);
        assert!(m
            .people
            .iter()
            .any(|p| p.role == PersonRole::Author && p.name == "Kentarou Miura"));
        assert!(m
            .people
            .iter()
            .any(|p| p.role == PersonRole::Artist && p.name == "Kentarou Miura"));
        assert!(m
            .cover_url
            .as_deref()
            .unwrap()
            .starts_with("https://uploads.mangadex.org/covers/"));
        assert_eq!(
            m.url.as_deref(),
            Some("https://mangadex.org/title/11111111-1111-1111-1111-111111111111")
        );
    }

    #[test]
    fn links_map_to_external_ids() {
        let ext = external_ids(berserk().attributes.links.as_ref());
        let find = |p: &str| ext.iter().find(|e| e.provider().as_str() == p);
        assert_eq!(find("mal").unwrap().value(), "2");
        assert_eq!(
            find("mal").unwrap().url(),
            Some("https://myanimelist.net/manga/2/")
        );
        assert_eq!(find("anilist").unwrap().value(), "30002");
        assert_eq!(
            find("anilist").unwrap().url(),
            Some("https://anilist.co/manga/30002/")
        );
        assert_eq!(find("kitsu").unwrap().value(), "12345");
        assert_eq!(find("bookwalker").unwrap().value(), "series/1234");
        assert!(find("bookwalker").unwrap().url().is_none());
        assert!(
            ext.iter().all(|e| e.provider().as_str() != "amz"),
            "unsupported sites dropped"
        );
    }

    #[test]
    fn feed_builds_volume_and_loose_chapters() {
        let data: super::super::response::FeedResponse = parse("chapter_feed.json");
        let tree = nodes(&data.data, "11111111-1111-1111-1111-111111111111");
        assert_eq!(tree.len(), 3, "volume 1, volume 2, loose side story");
        let vol1 = &tree[0];
        assert_eq!(vol1.kind, NodeKind::Volume);
        assert_eq!(vol1.number.as_deref(), Some("1"));
        assert_eq!(vol1.id, "11111111-1111-1111-1111-111111111111-vol-1");
        assert_eq!(vol1.children.len(), 2);
        let ch1 = &vol1.children[0];
        assert_eq!(ch1.id, "ch-0001");
        assert_eq!(ch1.kind, NodeKind::Chapter);
        assert_eq!(ch1.number.as_deref(), Some("1"));
        assert_eq!(ch1.title.as_deref(), Some("The Brand of the Sacrifice"));
        assert_eq!(ch1.release_date.as_deref(), Some("2018-01-01"));
        assert_eq!(ch1.page_count, Some(29));
        assert_eq!(tree[1].number.as_deref(), Some("2"));
        assert_eq!(
            tree[2].kind,
            NodeKind::Chapter,
            "volume-less chapter is top-level"
        );
        assert_eq!(
            tree[2].title.as_deref(),
            Some("Side Story: A Hunters Campfire")
        );
    }
}
