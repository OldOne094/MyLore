//! TMDB → domain normalization (MISSION-055).
//!
//! Pure functions mapping the serde response models into the unified domain
//! types (`ProviderCandidate`/`ProviderMedia`/`ProviderNode`/`ExternalId`). No
//! I/O here, so every mapping is unit-tested offline against recorded fixtures
//! (`tests/fixtures/tmdb/`). The provider id for TMDB titles is
//! `movie-<id>` / `tv-<id>`: TMDB ids are per-kind (movie 603 ≠ tv 603), so the
//! kind must be part of the identity to route `/movie/…` vs `/tv/…`.

use std::collections::HashSet;

use crate::domain::enums::{ContentType, MediaStatus, NodeKind, PersonRole};
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderPerson,
};
use crate::domain::value_objects::{ExternalId, ProviderId};

use super::response::{ExternalIds, MediaDetails, SearchResult, SeasonDetails};
use super::PROVIDER_ID;

/// Poster images are served at fixed sizes; `w500` is a good balance for list
/// and detail views. Backdrops use the original resolution (banners).
pub(crate) const POSTER_SIZE: &str = "w500";
pub(crate) const IMAGE_BASE: &str = "https://image.tmdb.org/t/p/";

pub(crate) fn poster_url(path: Option<&str>) -> Option<String> {
    path.map(|p| format!("{IMAGE_BASE}{POSTER_SIZE}{p}"))
}

pub(crate) fn backdrop_url(path: Option<&str>) -> Option<String> {
    path.map(|p| format!("{IMAGE_BASE}original{p}"))
}

/// Map the `/search/multi` `media_type` discriminator to a domain type.
pub(crate) fn content_type(media_type: Option<&str>) -> ContentType {
    match media_type {
        Some("movie") => ContentType::Movie,
        Some("tv") => ContentType::Tv,
        _ => ContentType::Other,
    }
}

pub(crate) fn pub_status(status: Option<&str>) -> MediaStatus {
    match status {
        Some("Released") | Some("Ended") => MediaStatus::Completed,
        Some("Returning Series") | Some("In Production") | Some("Post Production") => {
            MediaStatus::Ongoing
        }
        Some("Planned") | Some("Rumored") => MediaStatus::Announced,
        Some("Canceled") | Some("Cancelled") => MediaStatus::Cancelled,
        _ => MediaStatus::Unknown,
    }
}

/// The first four characters of a `YYYY-MM-DD` date as a year.
pub(crate) fn year(date: Option<&str>) -> Option<i32> {
    date.and_then(|d| d.get(..4)).and_then(|y| y.parse().ok())
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Map a search row to a candidate. Returns `None` for non-movie/tv rows
/// (people, companies, episodes).
pub(crate) fn candidate(row: &SearchResult) -> Option<ProviderCandidate> {
    let content_type = content_type(row.media_type.as_deref());
    if content_type == ContentType::Other {
        return None;
    }
    let main = row
        .title
        .clone()
        .or_else(|| row.name.clone())
        .or_else(|| row.original_title.clone())
        .or_else(|| row.original_name.clone())
        .unwrap_or_default();
    let is_movie = content_type == ContentType::Movie;
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: format!("{}-{}", kind_str(is_movie), row.id),
        title: main,
        content_type,
        release_year: year(
            row.release_date
                .as_deref()
                .or(row.first_air_date.as_deref()),
        ),
        cover_url: poster_url(row.poster_path.as_deref()),
        synopsis: clean(row.overview.clone()),
        external_ids: Vec::new(),
        url: Some(site_url(is_movie, row.id)),
    })
}

fn kind_str(is_movie: bool) -> &'static str {
    if is_movie {
        "movie"
    } else {
        "tv"
    }
}

fn site_url(is_movie: bool, id: i64) -> String {
    let kind = kind_str(is_movie);
    format!("https://www.themoviedb.org/{kind}/{id}")
}

/// The display title: localized `title` (movies) / `name` (TV), falling back to
/// the original-language title.
fn main_title(details: &MediaDetails) -> String {
    details
        .title
        .clone()
        .or_else(|| details.name.clone())
        .or_else(|| details.original_title.clone())
        .or_else(|| details.original_name.clone())
        .unwrap_or_default()
}

fn original_title(details: &MediaDetails) -> Option<String> {
    let main = main_title(details);
    details
        .original_title
        .clone()
        .or_else(|| details.original_name.clone())
        .filter(|o| !o.is_empty() && *o != main)
}

fn alt_titles(details: &MediaDetails) -> Vec<String> {
    let main = main_title(details);
    let original = original_title(details);
    let mut alts = Vec::new();
    for candidate in [details.title.clone(), details.name.clone()] {
        let Some(candidate) = candidate else { continue };
        if !candidate.is_empty()
            && candidate != main
            && Some(&candidate) != original.as_ref()
            && !alts.contains(&candidate)
        {
            alts.push(candidate);
        }
    }
    alts
}

/// Map full details to `ProviderMedia`. `is_movie` disambiguates the shared
/// `MediaDetails` shape (and the provider id prefix).
pub(crate) fn media(details: &MediaDetails, is_movie: bool) -> ProviderMedia {
    let start = if is_movie {
        details.release_date.clone()
    } else {
        details.first_air_date.clone()
    };
    ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: format!("{}-{}", kind_str(is_movie), details.id),
        title_main: main_title(details),
        title_original: original_title(details),
        alt_titles: alt_titles(details),
        content_type: if is_movie {
            ContentType::Movie
        } else {
            ContentType::Tv
        },
        format: None,
        pub_status: pub_status(details.status.as_deref()),
        synopsis: clean(details.overview.clone()),
        start_date: start.clone(),
        end_date: if is_movie {
            None
        } else {
            details.last_air_date.clone()
        },
        release_year: year(start.as_deref()),
        language: details.original_language.clone(),
        country: details
            .production_countries
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.iso_3166_1.clone()),
        content_rating: None,
        pages: None,
        duration_min: if is_movie {
            details.runtime.map(|r| r as u32)
        } else {
            details
                .episode_run_time
                .as_ref()
                .and_then(|run| run.first())
                .map(|r| *r as u32)
        },
        ep_count: if is_movie {
            None
        } else {
            details.number_of_episodes.map(|n| n as u32)
        },
        ch_count: None,
        cover_url: poster_url(details.poster_path.as_deref()),
        banner_url: backdrop_url(details.backdrop_path.as_deref()),
        url: clean(details.homepage.clone()).or_else(|| Some(site_url(is_movie, details.id))),
        people: people(details, is_movie),
        genres: details
            .genres
            .as_ref()
            .map(|genres| {
                genres
                    .iter()
                    .filter_map(|g| g.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        tags: Vec::new(),
        external_ids: external_ids(details.external_ids.as_ref()),
    }
}

/// People: directors/writers from `credits.crew`, then production companies as
/// Studio and (for TV) networks as Network. Cast has no fitting domain role, so
/// it is intentionally skipped. Entries are deduplicated per `(role, name)`.
pub(crate) fn people(details: &MediaDetails, is_movie: bool) -> Vec<ProviderPerson> {
    let mut out = Vec::new();
    let mut seen: HashSet<(PersonRole, String)> = HashSet::new();

    if let Some(credits) = &details.credits {
        for member in &credits.crew {
            let Some(name) = member.name.as_deref() else {
                continue;
            };
            let job = member.job.as_deref().unwrap_or("").to_ascii_lowercase();
            let role = if job == "director" {
                Some(PersonRole::Director)
            } else if job == "writer" || job == "screenplay" || job == "story" || job == "teleplay"
            {
                Some(PersonRole::Author)
            } else if job.contains("art") || job.contains("design") {
                Some(PersonRole::Artist)
            } else {
                None
            };
            if let Some(role) = role {
                push_people(&mut out, &mut seen, role, name);
            }
        }
    }

    if let Some(companies) = &details.production_companies {
        for company in companies {
            if let Some(name) = company.name.as_deref() {
                push_people(&mut out, &mut seen, PersonRole::Studio, name);
            }
        }
    }

    if !is_movie {
        if let Some(networks) = &details.networks {
            for network in networks {
                if let Some(name) = network.name.as_deref() {
                    push_people(&mut out, &mut seen, PersonRole::Network, name);
                }
            }
        }
    }

    out
}

fn push_people(
    out: &mut Vec<ProviderPerson>,
    seen: &mut HashSet<(PersonRole, String)>,
    role: PersonRole,
    name: &str,
) {
    let name = name.trim();
    if name.is_empty() || !seen.insert((role, name.to_string())) {
        return;
    }
    out.push(ProviderPerson {
        role,
        name: name.to_string(),
    });
}

/// Build one `Season` node with `Episode` children from `/tv/{id}/season/{n}`.
/// Season 0 (specials) is filtered by the caller; episodes are always regular.
pub(crate) fn season_tree(season: &SeasonDetails, show_id: i64) -> ProviderNode {
    let season_number = season.season_number.unwrap_or(0);
    let children = season
        .episodes
        .iter()
        .map(|ep| {
            let number = ep.episode_number.unwrap_or(0);
            ProviderNode {
                id: format!("tv-{show_id}-s{season_number}e{number}"),
                kind: NodeKind::Episode,
                position: number,
                number: ep.episode_number.map(|n| n.to_string()),
                title: clean(ep.name.clone()),
                release_date: clean(ep.air_date.clone()),
                duration_min: ep.runtime,
                page_count: None,
                synopsis: clean(ep.overview.clone()),
                is_special: false,
                children: Vec::new(),
            }
        })
        .collect();
    ProviderNode {
        id: format!("tv-{show_id}-s{season_number}"),
        kind: NodeKind::Season,
        position: season_number,
        number: Some(season_number.to_string()),
        title: clean(season.name.clone()),
        release_date: clean(season.air_date.clone()),
        duration_min: None,
        page_count: None,
        synopsis: clean(season.overview.clone()),
        is_special: false,
        children,
    }
}

/// Cross-provider ids from TMDB's `external_ids`. IMDb gets a canonical URL;
/// the rest carry no URL (no stable per-id link we trust).
pub(crate) fn external_ids(ext: Option<&ExternalIds>) -> Vec<ExternalId> {
    let mut out = Vec::new();
    let Some(ext) = ext else { return out };
    if let Some(id) = ext.imdb_id.as_deref().filter(|id| !id.is_empty()) {
        push_external(
            &mut out,
            "imdb",
            id,
            Some(format!("https://www.imdb.com/title/{id}/")),
        );
    }
    if let Some(id) = ext.tvdb_id {
        push_external(&mut out, "tvdb", &id.to_string(), None);
    }
    if let Some(id) = ext.tvrage_id {
        push_external(&mut out, "tvrage", &id.to_string(), None);
    }
    if let Some(id) = ext.wikidata_id.as_deref().filter(|id| !id.is_empty()) {
        push_external(&mut out, "wikidata", id, None);
    }
    out
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
    use crate::infrastructure::providers::test_support::tmdb_fixture;

    fn parse<T: serde::de::DeserializeOwned>(fixture: &str) -> T {
        serde_json::from_str(&tmdb_fixture(fixture)).expect("fixture parses")
    }

    fn movie() -> MediaDetails {
        parse("details_movie.json")
    }

    fn tv() -> MediaDetails {
        parse("details_tv.json")
    }

    #[test]
    fn content_type_maps_movie_and_tv_only() {
        assert_eq!(content_type(Some("movie")), ContentType::Movie);
        assert_eq!(content_type(Some("tv")), ContentType::Tv);
        assert_eq!(content_type(Some("person")), ContentType::Other);
        assert_eq!(content_type(None), ContentType::Other);
    }

    #[test]
    fn pub_status_maps_tmdb_values() {
        assert_eq!(pub_status(Some("Released")), MediaStatus::Completed);
        assert_eq!(pub_status(Some("Ended")), MediaStatus::Completed);
        assert_eq!(pub_status(Some("Returning Series")), MediaStatus::Ongoing);
        assert_eq!(pub_status(Some("In Production")), MediaStatus::Ongoing);
        assert_eq!(pub_status(Some("Planned")), MediaStatus::Announced);
        assert_eq!(pub_status(Some("Canceled")), MediaStatus::Cancelled);
        assert_eq!(pub_status(Some("Weird")), MediaStatus::Unknown);
    }

    #[test]
    fn search_rows_map_to_candidates_with_kind_prefixed_ids() {
        let data: super::super::response::SearchResponse = parse("search_multi.json");
        let hits: Vec<ProviderCandidate> = data.results.iter().filter_map(candidate).collect();
        assert_eq!(hits.len(), 3, "movie 603, tv 1396, movie 42");
        let matrix = hits.iter().find(|h| h.provider_id == "movie-603").unwrap();
        assert_eq!(matrix.content_type, ContentType::Movie);
        assert_eq!(matrix.title, "The Matrix");
        assert_eq!(matrix.release_year, Some(1999));
        assert!(matrix
            .cover_url
            .as_deref()
            .unwrap()
            .starts_with("https://image.tmdb.org/t/p/w500"));
        assert_eq!(
            matrix.url.as_deref(),
            Some("https://www.themoviedb.org/movie/603")
        );
        assert!(matrix.synopsis.as_deref().unwrap().contains("hacker"));
        let bb = hits.iter().find(|h| h.provider_id == "tv-1396").unwrap();
        assert_eq!(bb.content_type, ContentType::Tv);
        assert_eq!(bb.release_year, Some(2008));
        assert_eq!(
            bb.url.as_deref(),
            Some("https://www.themoviedb.org/tv/1396")
        );
        let sparse = hits.iter().find(|h| h.provider_id == "movie-42").unwrap();
        assert!(sparse.synopsis.is_none());
        assert!(sparse.cover_url.is_none());
        assert!(sparse.release_year.is_none());
    }

    #[test]
    fn movie_details_normalize_fully() {
        let m = media(&movie(), true);
        assert_eq!(m.provider_id, "movie-603");
        assert_eq!(m.title_main, "The Matrix");
        assert_eq!(
            m.title_original, None,
            "original_title duplicates the main title → dropped"
        );
        assert_eq!(m.content_type, ContentType::Movie);
        assert_eq!(m.pub_status, MediaStatus::Completed);
        assert_eq!(m.start_date.as_deref(), Some("1999-03-31"));
        assert_eq!(m.end_date, None);
        assert_eq!(m.release_year, Some(1999));
        assert_eq!(m.duration_min, Some(136));
        assert_eq!(m.ep_count, None);
        assert_eq!(m.country.as_deref(), Some("US"));
        assert_eq!(m.language.as_deref(), Some("en"));
        assert_eq!(m.genres, vec!["Action", "Thriller", "Science Fiction"]);
        assert_eq!(
            m.cover_url.as_deref().unwrap(),
            "https://image.tmdb.org/t/p/w500/f89U3ADr1oiB1s9GkdPOEpXUk5H.jpg"
        );
        assert!(m
            .banner_url
            .as_deref()
            .unwrap()
            .starts_with("https://image.tmdb.org/t/p/original"));
        assert_eq!(
            m.url.as_deref(),
            Some("http://www.warnerbros.com/movies/matrix/")
        );
    }

    #[test]
    fn movie_people_include_directors_writers_and_studio() {
        let people = people(&movie(), true);
        let roles: Vec<PersonRole> = people.iter().map(|p| p.role).collect();
        assert!(people
            .iter()
            .any(|p| p.role == PersonRole::Director && p.name == "Lana Wachowski"));
        assert!(people
            .iter()
            .any(|p| p.role == PersonRole::Author && p.name == "The Wachowskis"));
        assert!(people
            .iter()
            .any(|p| p.role == PersonRole::Studio && p.name == "Warner Bros. Pictures"));
        assert!(
            !roles.contains(&PersonRole::Network),
            "movies have no networks"
        );
        assert!(
            !people.iter().any(|p| p.name == "Joel Silver"),
            "producers are skipped"
        );
    }

    #[test]
    fn tv_details_normalize_fully() {
        let t = media(&tv(), false);
        assert_eq!(t.provider_id, "tv-1396");
        assert_eq!(t.title_main, "Breaking Bad");
        assert_eq!(t.content_type, ContentType::Tv);
        assert_eq!(t.pub_status, MediaStatus::Completed);
        assert_eq!(t.start_date.as_deref(), Some("2008-01-20"));
        assert_eq!(t.end_date.as_deref(), Some("2013-09-29"));
        assert_eq!(t.duration_min, Some(47), "episode_run_time[0]");
        assert_eq!(t.ep_count, Some(62));
        assert!(t
            .people
            .iter()
            .any(|p| p.role == PersonRole::Network && p.name == "AMC"));
        assert!(t
            .people
            .iter()
            .any(|p| p.role == PersonRole::Director && p.name == "Vince Gilligan"));
        assert!(t
            .people
            .iter()
            .any(|p| p.role == PersonRole::Studio && p.name == "Sony Pictures Television"));
    }

    #[test]
    fn season_tree_builds_season_with_episode_children() {
        let season: super::super::response::SeasonDetails = parse("season_1.json");
        let node = season_tree(&season, 1396);
        assert_eq!(node.kind, NodeKind::Season);
        assert_eq!(node.id, "tv-1396-s1");
        assert_eq!(node.title.as_deref(), Some("Season 1"));
        assert_eq!(node.children.len(), 2);
        let first = &node.children[0];
        assert_eq!(first.id, "tv-1396-s1e1");
        assert_eq!(first.kind, NodeKind::Episode);
        assert_eq!(first.number.as_deref(), Some("1"));
        assert_eq!(first.title.as_deref(), Some("Pilot"));
        assert_eq!(first.release_date.as_deref(), Some("2008-01-20"));
        assert_eq!(first.duration_min, Some(58));
        assert!(first.synopsis.as_deref().unwrap().contains("Walter White"));
        assert!(first.children.is_empty());
    }

    #[test]
    fn movie_external_ids_keep_imdb_and_wikidata() {
        let ext = external_ids(Some(&parse::<ExternalIds>("external_ids_movie.json")));
        let find = |p: &str| ext.iter().find(|e| e.provider().as_str() == p);
        assert_eq!(find("imdb").unwrap().value(), "tt0133093");
        assert_eq!(
            find("imdb").unwrap().url(),
            Some("https://www.imdb.com/title/tt0133093/")
        );
        assert_eq!(find("wikidata").unwrap().value(), "Q253732");
        assert!(find("tvdb").is_none());
    }

    #[test]
    fn tv_external_ids_include_tvdb_and_tvrage() {
        let ext = external_ids(Some(&parse::<ExternalIds>("external_ids_tv.json")));
        let find = |p: &str| ext.iter().find(|e| e.provider().as_str() == p);
        assert_eq!(find("imdb").unwrap().value(), "tt0903747");
        assert_eq!(find("tvdb").unwrap().value(), "81189");
        assert_eq!(find("tvrage").unwrap().value(), "18164");
    }
}
