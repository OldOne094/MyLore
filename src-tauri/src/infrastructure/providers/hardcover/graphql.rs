//! Hardcover GraphQL queries (MISSION-064).
//!
//! Hardcover is a Hasura GraphQL service (`api.hardcover.app/v1/graphql`). The
//! `search` root wraps Typesense and returns JSON blobs (`results`); details go
//! through the typed `books` root. Queries stay in one place so the request
//! shape is auditable against https://docs.hardcover.app.

/// `search(query, "Book", …)` — candidates. Only `query` is required; the
/// defaults match the website's book search. `results` is a JSON blob array,
/// parsed defensively in response.rs.
pub const SEARCH_QUERY: &str = r#"
query SearchBooks($query: String!, $per_page: Int, $page: Int) {
  search(query: $query, query_type: "Book", per_page: $per_page, page: $page) {
    ids
    results
  }
}
"#;

/// `books(where: {id: {_eq: $id}})` — full detail: category (→ content type),
/// cover image, contributions (authors), `cached_tags` (Genre bucket) and the
/// ISBNs living on editions.
pub const DETAILS_QUERY: &str = r#"
query BookDetails($id: Int!) {
  books(where: {id: {_eq: $id}}, limit: 1) {
    id
    title
    subtitle
    slug
    description
    release_date
    release_year
    pages
    book_category_id
    cached_tags
    image { url }
    contributions { author { name } }
    editions { isbn_10 isbn_13 }
  }
}
"#;
