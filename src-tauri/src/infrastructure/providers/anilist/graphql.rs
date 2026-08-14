//! AniList GraphQL queries (MISSION-054).
//!
//! Kept in one place so the request shape is auditable against
//! https://anilist.co/graphiql. Fields were chosen to exactly fill the domain
//! normalization (response.rs) — no over-fetching.

/// `search(query, type)` — the candidate list. `SEARCH_MATCH` sorts by
/// relevance; `type` is optional (null = both anime and manga).
pub const SEARCH_QUERY: &str = r#"
query Search($q: String, $type: MediaType, $page: Int, $perPage: Int) {
  Page(page: $page, perPage: $perPage) {
    media(search: $q, type: $type, sort: SEARCH_MATCH) {
      id
      type
      format
      countryOfOrigin
      title { romaji english native }
      coverImage { extraLarge large }
      description
      startDate { year }
      siteUrl
    }
  }
}
"#;

/// `Media(id)` — the full shape powering details, nodes (episode/chapter
/// counts), relations and external ids from a single request.
pub const DETAILS_QUERY: &str = r#"
query Details($id: Int) {
  Media(id: $id) {
    id
    type
    format
    countryOfOrigin
    title { romaji english native }
    coverImage { extraLarge large }
    bannerImage
    description
    startDate { year month day }
    endDate { year month day }
    status
    episodes
    chapters
    duration
    genres
    tags { name }
    studios { edges { isMain node { name } } }
    staff { edges { role node { name } } }
    relations { edges { relationType node { id title { romaji } } } }
    externalLinks { site id url }
    siteUrl
  }
}
"#;
