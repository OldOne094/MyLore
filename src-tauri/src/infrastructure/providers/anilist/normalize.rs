//! AniList → domain normalization (MISSION-054).
//!
//! Pure functions mapping the serde response models into the unified domain
//! types (`ProviderCandidate`/`ProviderMedia`/`ProviderNode`/`ProviderRelation`
//! /`ExternalId`). No I/O here, so every mapping is unit-tested offline against
//! recorded fixtures (`tests/fixtures/anilist/`).

use std::collections::HashSet;

use crate::domain::enums::{ContentType, MediaRelationKind, MediaStatus, NodeKind, PersonRole};
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderPerson, ProviderRelation,
};
use crate::domain::value_objects::{ExternalId, ProviderId};

use super::response::{ExternalLink, FuzzyDate, MediaFull, MediaSearch, Title};
use super::PROVIDER_ID;

/// Strip HTML tags and collapse whitespace (AniList descriptions are HTML).
pub(crate) fn strip_html(input: &str) -> String {
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

/// Map an AniList `(type, format, countryOfOrigin)` triple to a domain
/// content type. Manhwa/manhua are `MANGA` type distinguished by country of
/// origin; light novels are `MANGA` + `NOVEL` format.
pub(crate) fn content_type(
    media_type: Option<&str>,
    format: Option<&str>,
    country: Option<&str>,
) -> ContentType {
    let format = format.unwrap_or("");
    let country = country.unwrap_or("");
    match media_type {
        Some("ANIME") => match format {
            "TV" | "TV_SHORT" => ContentType::Anime,
            "MOVIE" => ContentType::Movie,
            _ => ContentType::Other,
        },
        Some("MANGA") => match format {
            "NOVEL" => ContentType::Novel,
            _ if country == "KR" => ContentType::Manhwa,
            _ if country == "CN" => ContentType::Manhua,
            _ => ContentType::Manga,
        },
        _ => ContentType::Other,
    }
}

pub(crate) fn pub_status(status: Option<&str>) -> MediaStatus {
    match status {
        Some("FINISHED") => MediaStatus::Completed,
        Some("RELEASING") => MediaStatus::Ongoing,
        Some("NOT_YET_RELEASED") => MediaStatus::Announced,
        Some("CANCELLED") => MediaStatus::Cancelled,
        Some("HIATUS") => MediaStatus::Hiatus,
        _ => MediaStatus::Unknown,
    }
}

/// A `FuzzyDate` → `YYYY-MM-DD`. Missing parts default to the earliest day so
/// the result is always a valid `DATE`-style string.
pub(crate) fn date_string(date: Option<&FuzzyDate>) -> Option<String> {
    let date = date?;
    let year = date.year?;
    let month = date.month.unwrap_or(1);
    let day = date.day.unwrap_or(1);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

pub(crate) fn main_title(title: &Title) -> String {
    title
        .romaji
        .clone()
        .or_else(|| title.english.clone())
        .or_else(|| title.native.clone())
        .unwrap_or_default()
}

pub(crate) fn original_title(title: &Title) -> Option<String> {
    title.native.clone()
}

pub(crate) fn alt_titles(title: &Title) -> Vec<String> {
    let main = main_title(title);
    let mut alts = Vec::new();
    if let Some(english) = &title.english {
        if !english.is_empty() && english != &main {
            alts.push(english.clone());
        }
    }
    alts
}

/// Map a search row to a candidate.
pub(crate) fn candidate(media: &MediaSearch) -> ProviderCandidate {
    ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: media.id.to_string(),
        title: main_title(&media.title),
        content_type: content_type(
            media.media_type.as_deref(),
            media.format.as_deref(),
            media.country_of_origin.as_deref(),
        ),
        release_year: media
            .start_date
            .as_ref()
            .and_then(|d| d.year)
            .map(|y| y as i32),
        cover_url: media
            .cover_image
            .as_ref()
            .and_then(|c| c.extra_large.clone().or_else(|| c.large.clone())),
        synopsis: media
            .description
            .as_deref()
            .map(strip_html)
            .filter(|s| !s.is_empty()),
        external_ids: Vec::new(),
        url: media.site_url.clone(),
    }
}

/// Map full media to `ProviderMedia` (the `MediaMeta` of the architecture).
pub(crate) fn media(full: &MediaFull) -> ProviderMedia {
    let is_anime = full.media_type.as_deref() == Some("ANIME");
    let content_type = content_type(
        full.media_type.as_deref(),
        full.format.as_deref(),
        full.country_of_origin.as_deref(),
    );
    ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: full.id.to_string(),
        title_main: main_title(&full.title),
        title_original: original_title(&full.title),
        alt_titles: alt_titles(&full.title),
        content_type,
        format: full.format.clone(),
        pub_status: pub_status(full.status.as_deref()),
        synopsis: full
            .description
            .as_deref()
            .map(strip_html)
            .filter(|s| !s.is_empty()),
        start_date: date_string(full.start_date.as_ref()),
        end_date: date_string(full.end_date.as_ref()),
        release_year: full
            .start_date
            .as_ref()
            .and_then(|d| d.year)
            .map(|y| y as i32),
        language: None,
        country: full.country_of_origin.clone(),
        content_rating: None,
        pages: None,
        duration_min: full.duration.map(|d| d as u32),
        ep_count: if is_anime {
            full.episodes.map(|e| e as u32)
        } else {
            None
        },
        ch_count: if is_anime {
            None
        } else {
            full.chapters.map(|c| c as u32)
        },
        cover_url: full
            .cover_image
            .as_ref()
            .and_then(|c| c.extra_large.clone().or_else(|| c.large.clone())),
        banner_url: full.banner_image.clone(),
        url: full.site_url.clone(),
        people: people(full),
        genres: full.genres.clone().unwrap_or_default(),
        tags: full
            .tags
            .as_ref()
            .map(|tags| tags.iter().filter_map(|t| t.name.clone()).collect())
            .unwrap_or_default(),
        external_ids: external_ids(full.external_links.as_deref().unwrap_or(&[])),
    }
}

/// Build a flat episode/chapter node list from AniList's counts. AniList has no
/// per-episode titles via this query, so each row carries its number and the
/// anime duration; position = 1-based count. Titles without counts yield no
/// nodes.
pub(crate) fn nodes(full: &MediaFull) -> Vec<ProviderNode> {
    let is_anime = full.media_type.as_deref() == Some("ANIME");
    let count = if is_anime {
        full.episodes
    } else {
        full.chapters
    };
    let Some(count) = count.filter(|c| *c > 0 && *c <= 100_000) else {
        return Vec::new();
    };
    let kind = if is_anime {
        NodeKind::Episode
    } else {
        NodeKind::Chapter
    };
    (1..=count)
        .map(|i| ProviderNode {
            id: format!("{}-{i}", full.id),
            kind,
            position: i,
            number: Some(i.to_string()),
            title: None,
            release_date: None,
            duration_min: if is_anime { full.duration } else { None },
            page_count: None,
            synopsis: None,
            is_special: false,
            children: Vec::new(),
        })
        .collect()
}

pub(crate) fn relations(full: &MediaFull) -> Vec<ProviderRelation> {
    let mut out = Vec::new();
    if let Some(edges) = full.relations.as_ref().and_then(|r| r.edges.as_ref()) {
        for edge in edges {
            out.push(ProviderRelation {
                to_provider: PROVIDER_ID.to_string(),
                to_id: edge.node.id.to_string(),
                relation: map_relation(edge.relation_type.as_deref()),
                title: edge
                    .node
                    .title
                    .romaji
                    .clone()
                    .or_else(|| edge.node.title.english.clone()),
            });
        }
    }
    out
}

fn map_relation(kind: Option<&str>) -> MediaRelationKind {
    match kind {
        Some("SEQUEL") => MediaRelationKind::Sequel,
        Some("PREQUEL") => MediaRelationKind::Prequel,
        Some("ADAPTATION") => MediaRelationKind::Adaptation,
        Some("SIDE_STORY") | Some("PARENT") | Some("SPIN_OFF") => MediaRelationKind::SpinOff,
        Some("CHARACTER") => MediaRelationKind::SameUniverse,
        _ => MediaRelationKind::Other,
    }
}

/// Cross-provider ids from `externalLinks`. Only links carrying a numeric id
/// are identity-bearing; url-only and non-identity sites are dropped. The
/// AniList site labels are mapped to canonical lowercase provider ids.
pub(crate) fn external_ids(links: &[ExternalLink]) -> Vec<ExternalId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for link in links {
        let Some(site) = link.site.as_deref() else {
            continue;
        };
        let Some(id) = link.id else { continue };
        let Some(provider) = site_to_provider(site) else {
            continue;
        };
        let value = id.to_string();
        if !seen.insert((provider.clone(), value.clone())) {
            continue;
        }
        if let Ok(provider) = ProviderId::new(provider) {
            if let Ok(external) = ExternalId::new(provider, value, link.url.clone()) {
                out.push(external);
            }
        }
    }
    out
}

/// Canonical provider id for an AniList `site` label; `None` for unknown or
/// non-identity sites (never invent a provider).
fn site_to_provider(site: &str) -> Option<String> {
    let id = match site {
        "MAL" | "MyAnimeList" => "mal",
        "AniDB" => "anidb",
        "TMDB" => "tmdb",
        "IMDb" => "imdb",
        "Kitsu" => "kitsu",
        "Anime-Planet" => "anime_planet",
        "MangaDex" => "mangadex",
        "Bangumi" => "bangumi",
        "ANN" | "Anime News Network" => "ann",
        "Twitter" | "Official Site" | "Crunchyroll" | "Netflix" | "Funimation" | "Amazon"
        | "Hulu" | "Disney" | "Wakanim" | "Pixiv" | "Twitch" | "Tumblr" | "Facebook"
        | "Instagram" | "YouTube" | "Reddit" | "Google Play" | "AniList" => return None,
        _ => return None,
    };
    Some(id.to_string())
}

/// Studio (main) + staff credits mapped to domain person roles. Unknown roles
/// are dropped rather than forced into `PersonRole::Other`.
pub(crate) fn people(full: &MediaFull) -> Vec<ProviderPerson> {
    let mut out = Vec::new();
    if let Some(edges) = full.studios.as_ref().and_then(|s| s.edges.as_ref()) {
        for edge in edges {
            if edge.is_main == Some(true) {
                if let Some(name) = edge.node.name.as_deref().filter(|n| !n.is_empty()) {
                    out.push(ProviderPerson {
                        role: PersonRole::Studio,
                        name: name.to_string(),
                    });
                }
            }
        }
    }
    if let Some(edges) = full.staff.as_ref().and_then(|s| s.edges.as_ref()) {
        for edge in edges {
            let Some(name) = edge.node.name.as_deref().filter(|n| !n.is_empty()) else {
                continue;
            };
            let Some(role) = map_staff_role(edge.role.as_deref()) else {
                continue;
            };
            out.push(ProviderPerson {
                role,
                name: name.to_string(),
            });
        }
    }
    out
}

fn map_staff_role(role: Option<&str>) -> Option<PersonRole> {
    let role = role?.to_ascii_lowercase();
    if role.contains("author")
        || role.contains("writer")
        || role.contains("story")
        || role.contains("producer")
    {
        Some(PersonRole::Author)
    } else if role.contains("art")
        || role.contains("illustrator")
        || role.contains("character design")
    {
        Some(PersonRole::Artist)
    } else if role.contains("director") {
        Some(PersonRole::Director)
    } else if role.contains("studio") || role.contains("animation") {
        Some(PersonRole::Studio)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
        serde_json::from_str(&fixture("anilist", name)).expect("fixture parses")
    }

    fn details(name: &str) -> MediaFull {
        let data: super::super::response::Envelope<super::super::response::DetailsData> =
            parse(name);
        data.data.and_then(|d| d.media).expect("media present")
    }

    #[test]
    fn search_row_maps_to_candidate() {
        let data: super::super::response::Envelope<super::super::response::SearchData> =
            parse("search_anime.json");
        let row = &data.data.expect("search data present").page.media[0];
        let candidate = candidate(row);
        assert_eq!(candidate.provider, "anilist");
        assert_eq!(candidate.provider_id, "1");
        assert_eq!(candidate.title, "Cowboy Bebop");
        assert_eq!(candidate.content_type, ContentType::Anime);
        assert_eq!(candidate.release_year, Some(1998));
        assert_eq!(
            candidate.cover_url.as_deref(),
            Some("https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx1.jpg")
        );
        assert_eq!(
            candidate.url.as_deref(),
            Some("https://anilist.co/anime/1/")
        );
        assert!(candidate
            .synopsis
            .as_deref()
            .unwrap_or("")
            .contains("ragtag"));
    }

    #[test]
    fn search_row_tolerates_missing_fields() {
        let data: super::super::response::Envelope<super::super::response::SearchData> =
            parse("search_anime.json");
        let row = &data.data.expect("search data present").page.media[1]; // One Piece: no description, no extraLarge cover
        let candidate = candidate(row);
        assert_eq!(candidate.title, "One Piece");
        assert!(candidate.synopsis.is_none());
        assert!(candidate.cover_url.is_some(), "falls back to large");
    }

    #[test]
    fn anime_details_normalize_fully() {
        let full = details("details_anime.json");
        let media = media(&full);
        assert_eq!(media.title_main, "Cowboy Bebop");
        assert_eq!(media.title_original.as_deref(), Some("カウボーイビバップ"));
        assert_eq!(media.alt_titles, Vec::<String>::new());
        assert_eq!(media.content_type, ContentType::Anime);
        assert_eq!(media.pub_status, MediaStatus::Completed);
        assert_eq!(media.start_date.as_deref(), Some("1998-04-03"));
        assert_eq!(media.end_date.as_deref(), Some("1999-04-26"));
        assert_eq!(media.release_year, Some(1998));
        assert_eq!(media.ep_count, Some(26));
        assert_eq!(media.duration_min, Some(24));
        assert_eq!(media.genres.len(), 5);
        assert_eq!(
            media.tags,
            vec!["Space".to_string(), "Bounty Hunters".to_string()]
        );
        assert!(media.synopsis.as_deref().unwrap_or("").contains("2071"));
        assert_eq!(media.country.as_deref(), Some("JP"));
    }

    #[test]
    fn people_map_studios_and_staff() {
        let full = details("details_anime.json");
        let people = people(&full);
        assert!(people
            .iter()
            .any(|p| p.role == PersonRole::Studio && p.name == "Sunrise"));
        assert!(!people
            .iter()
            .any(|p| p.role == PersonRole::Studio && p.name == "Bandai Visual"));
        assert!(people
            .iter()
            .any(|p| p.role == PersonRole::Director && p.name == "Shinichirou Watanabe"));
        assert!(people
            .iter()
            .any(|p| p.role == PersonRole::Artist && p.name == "Toshihiro Kawamoto"));
    }

    #[test]
    fn external_ids_map_known_sites_and_skip_others() {
        let full = details("details_anime.json");
        let ids = external_ids(full.external_links.as_deref().unwrap());
        assert_eq!(ids.len(), 4, "mal/anidb/imdb/tmdb; Official Site has no id");
        let find = |provider: &str| -> Option<String> {
            ids.iter()
                .find(|i| i.provider().as_str() == provider)
                .map(|i| i.value().to_string())
        };
        assert_eq!(find("mal").as_deref(), Some("1"));
        assert_eq!(find("anidb").as_deref(), Some("23"));
        assert_eq!(find("imdb").as_deref(), Some("151162"));
        assert_eq!(find("tmdb").as_deref(), Some("1429"));
    }

    #[test]
    fn light_novel_maps_to_novel_content_type() {
        let full = details("details_ln.json");
        let media = media(&full);
        assert_eq!(media.content_type, ContentType::Novel);
        assert_eq!(media.pub_status, MediaStatus::Ongoing);
        assert_eq!(media.ch_count, None, "chapters null for LN");
        assert_eq!(media.country.as_deref(), Some("JP"));
        assert_eq!(
            media.title_original.as_deref(),
            Some("ようこそ実力至上主義の教室へ")
        );
        assert!(media.people.iter().any(|p| p.role == PersonRole::Author));
    }

    #[test]
    fn nodes_build_flat_episode_list_for_anime() {
        let full = details("details_anime.json");
        let nodes = nodes(&full);
        assert_eq!(nodes.len(), 26);
        assert_eq!(nodes[0].kind, NodeKind::Episode);
        assert_eq!(nodes[0].position, 1);
        assert_eq!(nodes[0].number.as_deref(), Some("1"));
        assert_eq!(nodes[0].duration_min, Some(24));
        assert_eq!(nodes[0].id, "1-1");
        assert!(nodes.iter().all(|n| n.children.is_empty()));
    }

    #[test]
    fn no_nodes_without_counts() {
        let full = details("details_ln.json"); // chapters null
        assert!(nodes(&full).is_empty());
    }

    #[test]
    fn relations_map_to_domain_kinds() {
        let full = details("details_anime.json");
        let rels = relations(&full);
        assert_eq!(rels.len(), 2);
        assert_eq!(rels[0].relation, MediaRelationKind::Sequel);
        assert_eq!(rels[0].to_id, "30013");
        assert_eq!(rels[0].to_provider, "anilist");
        assert_eq!(rels[1].relation, MediaRelationKind::Adaptation);
    }

    #[test]
    fn strip_html_removes_tags_and_collapses_whitespace() {
        assert_eq!(strip_html("<p>Hi <b>there</b></p>"), "Hi there");
        assert_eq!(strip_html("plain text"), "plain text");
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn status_and_content_type_mapping() {
        assert_eq!(pub_status(Some("FINISHED")), MediaStatus::Completed);
        assert_eq!(pub_status(Some("RELEASING")), MediaStatus::Ongoing);
        assert_eq!(pub_status(Some("CANCELLED")), MediaStatus::Cancelled);
        assert_eq!(pub_status(Some("bogus")), MediaStatus::Unknown);

        assert_eq!(
            content_type(Some("ANIME"), Some("TV"), None),
            ContentType::Anime
        );
        assert_eq!(
            content_type(Some("ANIME"), Some("MOVIE"), None),
            ContentType::Movie
        );
        assert_eq!(
            content_type(Some("ANIME"), Some("SPECIAL"), None),
            ContentType::Other
        );
        assert_eq!(
            content_type(Some("MANGA"), Some("MANGA"), Some("JP")),
            ContentType::Manga
        );
        assert_eq!(
            content_type(Some("MANGA"), Some("MANGA"), Some("KR")),
            ContentType::Manhwa
        );
        assert_eq!(
            content_type(Some("MANGA"), Some("MANGA"), Some("CN")),
            ContentType::Manhua
        );
        assert_eq!(
            content_type(Some("MANGA"), Some("NOVEL"), Some("JP")),
            ContentType::Novel
        );
        assert_eq!(content_type(None, None, None), ContentType::Other);
    }
}
