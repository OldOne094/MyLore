//! Media aggregate repository (MISSION-019).
//!
//! Persists the `media` row plus its link sets (alt titles, person links,
//! genre/tag links, external ids, outgoing relations) in one transaction, and
//! exposes the library queries: `list` (filter/sort/paginate) and `search`
//! (FTS5 over both tokenizers, with the Arabic fold applied to the query).
//!
//! The FTS index is refreshed by triggers; this module never writes to it.

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::{QueryBuilder, Row};

use crate::error::AppError;
use crate::infrastructure::fts;

/// An alternative title of a media.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AltTitle {
    pub lang: String,
    pub title: String,
}

/// An external identity on a provider.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalId {
    pub provider: String,
    pub ext_id: String,
    pub url: Option<String>,
}

/// An outgoing relation from a media to another media.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaRelation {
    pub to_id: String,
    pub relation: String,
}

/// Full media aggregate: core columns plus every link set. Deserialize exists
/// so a trash before-image can be restored (MISSION-044).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaRecord {
    pub id: String,
    pub content_type: String,
    pub format: Option<String>,
    pub title_main: String,
    pub title_original: Option<String>,
    pub synopsis: Option<String>,
    pub pub_status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub release_year: Option<i64>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub content_rating: Option<String>,
    pub pages: Option<i64>,
    pub duration_min: Option<i64>,
    pub ep_count: Option<i64>,
    pub ch_count: Option<i64>,
    pub cover_asset_id: Option<String>,
    pub banner_asset_id: Option<String>,
    pub provider: Option<String>,
    pub provider_url: Option<String>,
    pub metadata_refreshed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub alt_titles: Vec<AltTitle>,
    pub people: Vec<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub external_ids: Vec<ExternalId>,
    pub relations: Vec<MediaRelation>,
}

/// Lightweight row for the library grid/list.
#[derive(Debug, Clone)]
pub struct MediaSummary {
    pub id: String,
    pub content_type: String,
    pub title_main: String,
    pub pub_status: String,
    pub release_year: Option<i64>,
    pub cover_asset_id: Option<String>,
    pub favorite: bool,
    pub updated_at: String,
}

/// Sort order for `list`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaSort {
    #[default]
    Title,
    CreatedAt,
    UpdatedAt,
    ReleaseYear,
}

/// Filtering and pagination options for `list`.
#[derive(Debug, Default, Clone)]
pub struct MediaFilter {
    pub content_type: Option<String>,
    pub format: Option<String>,
    pub pub_status: Option<String>,
    pub genre: Option<String>,
    pub tag: Option<String>,
    pub year: Option<i64>,
    pub favorite: Option<bool>,
    pub search: Option<String>,
    pub sort: MediaSort,
    pub ascending: bool,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// One selectable facet value (`genre`/`tag` rows carry an id + display name).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FacetOption {
    pub id: String,
    pub name: String,
}

/// Distinct filter values present in the library, for the filter panel.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct MediaFacets {
    pub formats: Vec<String>,
    pub genres: Vec<FacetOption>,
    pub tags: Vec<FacetOption>,
    pub years: Vec<i64>,
}

const MEDIA_COLUMNS: &str = "id, content_type, format, title_main, title_original, synopsis, \
     pub_status, start_date, end_date, release_year, language, country, content_rating, pages, \
     duration_min, ep_count, ch_count, cover_asset_id, banner_asset_id, provider, provider_url, \
     metadata_refreshed_at, created_at, updated_at";

/// Read the full aggregate for one media (or `None`).
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<MediaRecord>, AppError> {
    let row = sqlx::query(&format!("SELECT {MEDIA_COLUMNS} FROM media WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let media = row_to_record(row);

    let alt_titles = sqlx::query_as::<_, (String, String)>(
        "SELECT lang, title FROM media_alt_title WHERE media_id = ? ORDER BY title",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(lang, title)| AltTitle { lang, title })
    .collect();

    let people = sqlx::query_as::<_, (String,)>(
        "SELECT person_id FROM media_person WHERE media_id = ? ORDER BY person_id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(person_id,)| person_id)
    .collect();

    let genres = sqlx::query_as::<_, (String,)>(
        "SELECT genre_id FROM media_genre WHERE media_id = ? ORDER BY genre_id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(genre_id,)| genre_id)
    .collect();

    let tags = sqlx::query_as::<_, (String,)>(
        "SELECT tag_id FROM media_tag WHERE media_id = ? ORDER BY tag_id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(tag_id,)| tag_id)
    .collect();

    let external_ids = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT provider, ext_id, url FROM media_external_id WHERE media_id = ? ORDER BY provider",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(provider, ext_id, url)| ExternalId {
        provider,
        ext_id,
        url,
    })
    .collect();

    let relations = sqlx::query_as::<_, (String, String)>(
        "SELECT to_id, relation FROM media_relation WHERE from_id = ? ORDER BY relation",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(to_id, relation)| MediaRelation { to_id, relation })
    .collect();

    Ok(Some(MediaRecord {
        alt_titles,
        people,
        genres,
        tags,
        external_ids,
        relations,
        ..media
    }))
}

/// Insert a full aggregate (media row + links) in one transaction.
pub async fn create(pool: &SqlitePool, media: &MediaRecord) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    sqlx::query(&format!(
        "INSERT INTO media ({MEDIA_COLUMNS}) VALUES \
         (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ))
    .bind(&media.id)
    .bind(&media.content_type)
    .bind(&media.format)
    .bind(&media.title_main)
    .bind(&media.title_original)
    .bind(&media.synopsis)
    .bind(&media.pub_status)
    .bind(&media.start_date)
    .bind(&media.end_date)
    .bind(media.release_year)
    .bind(&media.language)
    .bind(&media.country)
    .bind(&media.content_rating)
    .bind(media.pages)
    .bind(media.duration_min)
    .bind(media.ep_count)
    .bind(media.ch_count)
    .bind(&media.cover_asset_id)
    .bind(&media.banner_asset_id)
    .bind(&media.provider)
    .bind(&media.provider_url)
    .bind(&media.metadata_refreshed_at)
    .bind(&media.created_at)
    .bind(&media.updated_at)
    .execute(&mut *tx)
    .await?;

    insert_links(&mut tx, media).await?;

    tx.commit().await?;
    Ok(())
}

/// Replace the aggregate (row + links) in one transaction. The record must
/// contain every link set; sets are rewritten wholesale.
pub async fn update(pool: &SqlitePool, media: &MediaRecord) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE media SET
           content_type = ?, format = ?, title_main = ?, title_original = ?, synopsis = ?,
           pub_status = ?, start_date = ?, end_date = ?, release_year = ?, language = ?,
           country = ?, content_rating = ?, pages = ?, duration_min = ?, ep_count = ?,
           ch_count = ?, cover_asset_id = ?, banner_asset_id = ?, provider = ?,
           provider_url = ?, metadata_refreshed_at = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&media.content_type)
    .bind(&media.format)
    .bind(&media.title_main)
    .bind(&media.title_original)
    .bind(&media.synopsis)
    .bind(&media.pub_status)
    .bind(&media.start_date)
    .bind(&media.end_date)
    .bind(media.release_year)
    .bind(&media.language)
    .bind(&media.country)
    .bind(&media.content_rating)
    .bind(media.pages)
    .bind(media.duration_min)
    .bind(media.ep_count)
    .bind(media.ch_count)
    .bind(&media.cover_asset_id)
    .bind(&media.banner_asset_id)
    .bind(&media.provider)
    .bind(&media.provider_url)
    .bind(&media.metadata_refreshed_at)
    .bind(&media.updated_at)
    .bind(&media.id)
    .execute(&mut *tx)
    .await?;

    for sql in [
        "DELETE FROM media_alt_title WHERE media_id = ?",
        "DELETE FROM media_person WHERE media_id = ?",
        "DELETE FROM media_genre WHERE media_id = ?",
        "DELETE FROM media_tag WHERE media_id = ?",
        "DELETE FROM media_external_id WHERE media_id = ?",
        "DELETE FROM media_relation WHERE from_id = ?",
    ] {
        sqlx::query(sql).bind(&media.id).execute(&mut *tx).await?;
    }

    insert_links(&mut tx, media).await?;

    tx.commit().await?;
    Ok(())
}

/// Delete a media; the FTS index and all aggregates cascade via FKs.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM media WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Resolve a media by a provider identity (dedup / REQ-MEDIA-005).
pub async fn find_by_external_id(
    pool: &SqlitePool,
    provider: &str,
    ext_id: &str,
) -> Result<Option<String>, AppError> {
    let result: Option<(String,)> =
        sqlx::query_as("SELECT media_id FROM media_external_id WHERE provider = ? AND ext_id = ?")
            .bind(provider)
            .bind(ext_id)
            .fetch_optional(pool)
            .await?;
    Ok(result.map(|(media_id,)| media_id))
}

/// Library listing with filter/sort/pagination.
pub async fn list(pool: &SqlitePool, filter: &MediaFilter) -> Result<Vec<MediaSummary>, AppError> {
    let mut qb = QueryBuilder::new(
        "SELECT m.id, m.content_type, m.title_main, m.pub_status, m.release_year, \
         m.cover_asset_id, COALESCE(r.favorite, 0) AS favorite, m.updated_at \
         FROM media m LEFT JOIN review r ON r.media_id = m.id",
    );

    if let Some(genre) = &filter.genre {
        qb.push(" JOIN media_genre mg ON mg.media_id = m.id AND mg.genre_id = ");
        qb.push_bind(genre);
    }
    if let Some(tag) = &filter.tag {
        qb.push(" JOIN media_tag mt ON mt.media_id = m.id AND mt.tag_id = ");
        qb.push_bind(tag);
    }

    push_where(&mut qb, filter);

    qb.push(" ORDER BY ");
    match filter.sort {
        MediaSort::Title => qb.push("m.title_main COLLATE NOCASE"),
        MediaSort::CreatedAt => qb.push("m.created_at"),
        MediaSort::UpdatedAt => qb.push("m.updated_at"),
        MediaSort::ReleaseYear => qb.push("m.release_year"),
    };
    qb.push(if filter.ascending { " ASC" } else { " DESC" });

    if let Some(limit) = filter.limit {
        qb.push(" LIMIT ").push_bind(limit);
        if let Some(offset) = filter.offset {
            qb.push(" OFFSET ").push_bind(offset);
        }
    }

    let rows = qb.build().fetch_all(pool).await?;
    Ok(rows.into_iter().map(row_to_summary).collect())
}

/// Count rows matching a filter (for pagination).
pub async fn count(pool: &SqlitePool, filter: &MediaFilter) -> Result<i64, AppError> {
    let mut qb = QueryBuilder::new("SELECT COUNT(DISTINCT m.id) FROM media m");

    if let Some(genre) = &filter.genre {
        qb.push(" JOIN media_genre mg ON mg.media_id = m.id AND mg.genre_id = ");
        qb.push_bind(genre);
    }
    if let Some(tag) = &filter.tag {
        qb.push(" JOIN media_tag mt ON mt.media_id = m.id AND mt.tag_id = ");
        qb.push_bind(tag);
    }
    if filter.favorite.is_some() {
        qb.push(" LEFT JOIN review r ON r.media_id = m.id");
    }

    push_where(&mut qb, filter);

    let row = qb.build().fetch_one(pool).await?;
    let n: i64 = row.get(0);
    Ok(n)
}

/// Distinct format/genre/tag/year values present in the library, for the
/// filter panel. Every value comes from actual rows so options never drift
/// from data.
pub async fn facets(pool: &SqlitePool) -> Result<MediaFacets, AppError> {
    let formats = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT format FROM media WHERE format IS NOT NULL ORDER BY format COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(format,)| format)
    .collect();

    let genres = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM genre ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, name)| FacetOption { id, name })
    .collect();

    let tags = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM tag WHERE scope = 'domain' ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, name)| FacetOption { id, name })
    .collect();

    let years = sqlx::query_as::<_, (i64,)>(
        "SELECT DISTINCT release_year FROM media WHERE release_year IS NOT NULL ORDER BY release_year DESC",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(year,)| year)
    .collect();

    Ok(MediaFacets {
        formats,
        genres,
        tags,
        years,
    })
}

/// Resolve a personal tag by exact name (or `None`). Personal tags are
/// user-created labels (tag.scope = 'personal'); bulk tagging reuses the row
/// when one already exists.
pub async fn resolve_personal_tag(
    pool: &SqlitePool,
    name: &str,
) -> Result<Option<String>, AppError> {
    let row =
        sqlx::query_as::<_, (String,)>("SELECT id FROM tag WHERE scope = 'personal' AND name = ?")
            .bind(name)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(id,)| id))
}

/// Create a personal tag row (ids are minted by the caller, e.g. `tag-{uuid}`).
pub async fn create_personal_tag(pool: &SqlitePool, id: &str, name: &str) -> Result<(), AppError> {
    sqlx::query("INSERT INTO tag (id, name, scope) VALUES (?, ?, 'personal')")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

/// Link a tag to many media rows in one transaction; already-linked pairs are
/// ignored. Resolves with the number of new links.
pub async fn add_tag_to_many(
    pool: &SqlitePool,
    tag_id: &str,
    media_ids: &[String],
) -> Result<usize, AppError> {
    let mut tx = pool.begin().await?;
    let mut added = 0usize;
    for media_id in media_ids {
        let res = sqlx::query("INSERT OR IGNORE INTO media_tag (media_id, tag_id) VALUES (?, ?)")
            .bind(media_id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await?;
        added += res.rows_affected() as usize;
    }
    tx.commit().await?;
    Ok(added)
}

/// Full-text search over both tokenizers, best matches first.
///
/// The query is folded like the index (lowercase + Arabic normalization), then
/// run against `media_fts` (unicode61, prefix terms) and `media_fts_cjk`
/// (trigram, phrase). Results are deduped, unicode61 hits first.
pub async fn search(pool: &SqlitePool, query: &str) -> Result<Vec<MediaSummary>, AppError> {
    let normalized = fts::normalize_query(query.trim());
    if normalized.is_empty() {
        return Ok(Vec::new());
    }

    let mut hits: Vec<MediaSummary> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for row in search_unicode61(pool, &normalized).await? {
        let summary = row_to_summary(row);
        seen.insert(summary.id.clone());
        hits.push(summary);
    }

    for row in search_trigram(pool, &normalized).await? {
        let summary = row_to_summary(row);
        if seen.insert(summary.id.clone()) {
            hits.push(summary);
        }
    }

    Ok(hits)
}

async fn search_unicode61(pool: &SqlitePool, normalized: &str) -> Result<Vec<SqliteRow>, AppError> {
    let terms: Vec<String> = normalized
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "")))
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let match_query = terms.join(" ");

    let rows = sqlx::query(
        "SELECT m.id, m.content_type, m.title_main, m.pub_status, m.release_year, \
         m.cover_asset_id, COALESCE(r.favorite, 0) AS favorite, m.updated_at \
         FROM media_fts JOIN media m ON m.rowid = media_fts.rowid \
         LEFT JOIN review r ON r.media_id = m.id \
         WHERE media_fts MATCH ? ORDER BY rank",
    )
    .bind(match_query)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn search_trigram(pool: &SqlitePool, normalized: &str) -> Result<Vec<SqliteRow>, AppError> {
    // trigram ignores query tokens shorter than 3 chars; the whole normalized
    // string is one phrase, giving substring-style matching.
    let phrase = format!("\"{}\"", normalized.replace('"', ""));
    let rows = sqlx::query(
        "SELECT m.id, m.content_type, m.title_main, m.pub_status, m.release_year, \
         m.cover_asset_id, COALESCE(r.favorite, 0) AS favorite, m.updated_at \
         FROM media_fts_cjk JOIN media m ON m.rowid = media_fts_cjk.rowid \
         LEFT JOIN review r ON r.media_id = m.id \
         WHERE media_fts_cjk MATCH ? ORDER BY rank",
    )
    .bind(phrase)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

fn push_where<'args>(qb: &mut QueryBuilder<'args, sqlx::Sqlite>, filter: &'args MediaFilter) {
    let mut pushed = false;
    let mut push = |qb: &mut QueryBuilder<'args, sqlx::Sqlite>, cond: String| {
        qb.push(if pushed { " AND " } else { " WHERE " });
        qb.push(cond);
        pushed = true;
    };

    if let Some(content_type) = &filter.content_type {
        push(qb, "m.content_type = ".to_string());
        qb.push_bind(content_type);
    }
    if let Some(format) = &filter.format {
        push(qb, "m.format = ".to_string());
        qb.push_bind(format);
    }
    if let Some(pub_status) = &filter.pub_status {
        push(qb, "m.pub_status = ".to_string());
        qb.push_bind(pub_status);
    }
    if let Some(year) = filter.year {
        push(qb, "m.release_year = ".to_string());
        qb.push_bind(year);
    }
    match filter.favorite {
        Some(true) => push(qb, "r.favorite = 1".to_string()),
        Some(false) => push(qb, "COALESCE(r.favorite, 0) = 0".to_string()),
        None => {}
    }
    if let Some(search) = &filter.search {
        let like = format!("%{}%", search.to_lowercase());
        push(qb, "lower(m.title_main) LIKE ".to_string());
        qb.push_bind(like);
    }
}

fn row_to_record(row: SqliteRow) -> MediaRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    MediaRecord {
        id: get(0).expect("id"),
        content_type: get(1).expect("content_type"),
        format: get(2),
        title_main: get(3).expect("title_main"),
        title_original: get(4),
        synopsis: get(5),
        pub_status: get(6).expect("pub_status"),
        start_date: get(7),
        end_date: get(8),
        release_year: row.get(9),
        language: get(10),
        country: get(11),
        content_rating: get(12),
        pages: row.get(13),
        duration_min: row.get(14),
        ep_count: row.get(15),
        ch_count: row.get(16),
        cover_asset_id: get(17),
        banner_asset_id: get(18),
        provider: get(19),
        provider_url: get(20),
        metadata_refreshed_at: get(21),
        created_at: get(22).expect("created_at"),
        updated_at: get(23).expect("updated_at"),
        alt_titles: Vec::new(),
        people: Vec::new(),
        genres: Vec::new(),
        tags: Vec::new(),
        external_ids: Vec::new(),
        relations: Vec::new(),
    }
}

fn row_to_summary(row: SqliteRow) -> MediaSummary {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    MediaSummary {
        id: get(0).expect("id"),
        content_type: get(1).expect("content_type"),
        title_main: get(2).expect("title_main"),
        pub_status: get(3).expect("pub_status"),
        release_year: row.get(4),
        cover_asset_id: get(5),
        favorite: row.get::<i64, _>(6) != 0,
        updated_at: get(7).expect("updated_at"),
    }
}

async fn insert_links<'e>(
    tx: &mut sqlx::Transaction<'e, sqlx::Sqlite>,
    media: &MediaRecord,
) -> Result<(), AppError> {
    for alt in &media.alt_titles {
        sqlx::query("INSERT INTO media_alt_title (media_id, lang, title) VALUES (?, ?, ?)")
            .bind(&media.id)
            .bind(&alt.lang)
            .bind(&alt.title)
            .execute(&mut **tx)
            .await?;
    }
    for person_id in &media.people {
        sqlx::query("INSERT INTO media_person (media_id, person_id) VALUES (?, ?)")
            .bind(&media.id)
            .bind(person_id)
            .execute(&mut **tx)
            .await?;
    }
    for genre_id in &media.genres {
        sqlx::query("INSERT INTO media_genre (media_id, genre_id) VALUES (?, ?)")
            .bind(&media.id)
            .bind(genre_id)
            .execute(&mut **tx)
            .await?;
    }
    for tag_id in &media.tags {
        sqlx::query("INSERT INTO media_tag (media_id, tag_id) VALUES (?, ?)")
            .bind(&media.id)
            .bind(tag_id)
            .execute(&mut **tx)
            .await?;
    }
    for ext in &media.external_ids {
        sqlx::query(
            "INSERT INTO media_external_id (media_id, provider, ext_id, url) VALUES (?, ?, ?, ?)",
        )
        .bind(&media.id)
        .bind(&ext.provider)
        .bind(&ext.ext_id)
        .bind(&ext.url)
        .execute(&mut **tx)
        .await?;
    }
    for rel in &media.relations {
        sqlx::query("INSERT INTO media_relation (from_id, to_id, relation) VALUES (?, ?, ?)")
            .bind(&media.id)
            .bind(&rel.to_id)
            .bind(&rel.relation)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::repositories::review;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn ensure_person(pool: &SqlitePool) {
        sqlx::query("INSERT INTO person (id, name, role) VALUES ('p-1', 'Test Author', 'author')")
            .execute(pool)
            .await
            .expect("seed person");
    }

    fn sample_media(id: &str, title: &str) -> MediaRecord {
        MediaRecord {
            id: id.to_string(),
            content_type: "novel".to_string(),
            format: Some("light_novel".to_string()),
            title_main: title.to_string(),
            title_original: None,
            synopsis: Some("A test synopsis".to_string()),
            pub_status: "ongoing".to_string(),
            start_date: Some("2025-01-01".to_string()),
            end_date: None,
            release_year: Some(2025),
            language: Some("ja".to_string()),
            country: None,
            content_rating: None,
            pages: None,
            duration_min: None,
            ep_count: None,
            ch_count: Some(120),
            cover_asset_id: None,
            banner_asset_id: None,
            provider: Some("anilist".to_string()),
            provider_url: None,
            metadata_refreshed_at: None,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
            alt_titles: Vec::new(),
            people: Vec::new(),
            genres: Vec::new(),
            tags: Vec::new(),
            external_ids: Vec::new(),
            relations: Vec::new(),
        }
    }

    fn with_links(mut m: MediaRecord) -> MediaRecord {
        m.alt_titles.push(AltTitle {
            lang: "ja".into(),
            title: "??????".into(),
        });
        m.people.push("p-1".to_string());
        m.genres.push("fantasy".to_string());
        m.tags.push("isekai".to_string());
        m.external_ids.push(ExternalId {
            provider: "anilist".into(),
            ext_id: "42".into(),
            url: Some("https://anilist.co/anime/42".into()),
        });
        m
    }

    #[tokio::test]
    async fn create_and_get_roundtrips_full_aggregate() {
        let (pool, path) = migrated_pool("media_repo_roundtrip.db").await;
        ensure_person(&pool).await;
        let media = with_links(sample_media("m-1", "Sword of the Dawn"));
        create(&pool, &media).await.expect("create");

        let got = get(&pool, "m-1").await.expect("get").expect("exists");
        assert_eq!(got.title_main, "Sword of the Dawn");
        assert_eq!(got.content_type, "novel");
        assert_eq!(got.ch_count, Some(120));
        assert_eq!(got.alt_titles.len(), 1);
        assert_eq!(got.alt_titles[0].lang, "ja");
        assert_eq!(got.people, vec!["p-1".to_string()]);
        assert_eq!(got.genres, vec!["fantasy".to_string()]);
        assert_eq!(got.tags, vec!["isekai".to_string()]);
        assert_eq!(got.external_ids.len(), 1);
        assert_eq!(got.external_ids[0].ext_id, "42");

        assert!(get(&pool, "missing").await.expect("get").is_none());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn update_replaces_link_sets() {
        let (pool, path) = migrated_pool("media_repo_update.db").await;
        ensure_person(&pool).await;
        let mut media = with_links(sample_media("m-1", "Sword of the Dawn"));
        create(&pool, &media).await.expect("create");

        media.title_main = "Dawn of the Sword".into();
        media.updated_at = "2026-02-01".into();
        media.alt_titles.clear();
        media.genres.push("action".to_string());
        media.tags.clear();
        update(&pool, &media).await.expect("update");

        let got = get(&pool, "m-1").await.expect("get").unwrap();
        assert_eq!(got.title_main, "Dawn of the Sword");
        assert_eq!(got.updated_at, "2026-02-01");
        assert!(got.alt_titles.is_empty(), "alt titles replaced");
        assert_eq!(got.genres.len(), 2, "genres extended");
        assert!(got.tags.is_empty(), "tags replaced");
        assert_eq!(got.people, vec!["p-1".to_string()], "people kept");
        assert_eq!(got.external_ids.len(), 1, "external ids kept");

        // The FTS index must follow the updated title.
        let hits = search(&pool, "dawn").await.expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "m-1");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn find_by_external_id_resolves_dedup_identity() {
        let (pool, path) = migrated_pool("media_repo_extid.db").await;
        ensure_person(&pool).await;
        let media = with_links(sample_media("m-1", "Sword"));
        create(&pool, &media).await.expect("create");

        let id = find_by_external_id(&pool, "anilist", "42")
            .await
            .expect("find");
        assert_eq!(id.as_deref(), Some("m-1"));

        let missing = find_by_external_id(&pool, "tmdb", "42")
            .await
            .expect("find");
        assert_eq!(missing, None, "unknown provider must not match");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn delete_cascades_and_removes_from_fts() {
        let (pool, path) = migrated_pool("media_repo_delete.db").await;
        ensure_person(&pool).await;
        let media = with_links(sample_media("m-1", "Doomed"));
        create(&pool, &media).await.expect("create");
        assert_eq!(search(&pool, "doomed").await.expect("search").len(), 1);

        delete(&pool, "m-1").await.expect("delete");

        assert!(get(&pool, "m-1").await.expect("get").is_none());
        assert!(search(&pool, "doomed").await.expect("search").is_empty());
        for (sql, name) in [
            ("SELECT COUNT(*) FROM media_alt_title", "alt titles"),
            ("SELECT COUNT(*) FROM media_person", "media_person"),
            ("SELECT COUNT(*) FROM media_genre", "media_genre"),
            ("SELECT COUNT(*) FROM media_tag", "media_tag"),
            ("SELECT COUNT(*) FROM media_external_id", "external ids"),
        ] {
            let (n,): (i64,) = sqlx::query_as(sql).fetch_one(&pool).await.unwrap();
            assert_eq!(n, 0, "{name} should cascade");
        }
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn list_filters_sorts_and_favorites() {
        let (pool, path) = migrated_pool("media_repo_list.db").await;
        let mut a = sample_media("m-a", "Alpha");
        a.content_type = "manga".into();
        create(&pool, &a).await.expect("create a");
        let mut b = sample_media("m-b", "Bravo");
        b.content_type = "manga".into();
        b.genres.push("fantasy".into());
        create(&pool, &b).await.expect("create b");
        let mut c = sample_media("m-c", "Charlie");
        c.content_type = "anime".into();
        create(&pool, &c).await.expect("create c");
        review::upsert(
            &pool,
            &review::ReviewRecord {
                media_id: "m-b".into(),
                rating: Some(9),
                review: None,
                short_review: None,
                notes: None,
                favorite: true,
                is_spoiler: false,
                created_at: "2026-01-01".into(),
                updated_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("review");

        let all = list(
            &pool,
            &MediaFilter {
                sort: MediaSort::Title,
                ascending: true,
                ..Default::default()
            },
        )
        .await
        .expect("list");
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].title_main, "Alpha", "title ascending");

        let mangas = list(
            &pool,
            &MediaFilter {
                content_type: Some("manga".into()),
                ..Default::default()
            },
        )
        .await
        .expect("list mangas");
        assert_eq!(mangas.len(), 2);

        let fantasy = list(
            &pool,
            &MediaFilter {
                genre: Some("fantasy".into()),
                ..Default::default()
            },
        )
        .await
        .expect("list genre");
        assert_eq!(fantasy.len(), 1);
        assert_eq!(fantasy[0].id, "m-b");

        let favorites = list(
            &pool,
            &MediaFilter {
                favorite: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("list favorites");
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].id, "m-b");
        assert!(favorites[0].favorite, "summary carries the favorite flag");

        let count = count(
            &pool,
            &MediaFilter {
                content_type: Some("manga".into()),
                ..Default::default()
            },
        )
        .await
        .expect("count");
        assert_eq!(count, 2);
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn list_paginates() {
        let (pool, path) = migrated_pool("media_repo_page.db").await;
        for (i, id) in ["m-1", "m-2", "m-3", "m-4", "m-5"].iter().enumerate() {
            let mut m = sample_media(id, &format!("Title {i:02}"));
            m.created_at = format!("2026-01-{:02}", i + 1);
            m.updated_at = m.created_at.clone();
            create(&pool, &m).await.expect("create");
        }

        let page = list(
            &pool,
            &MediaFilter {
                sort: MediaSort::CreatedAt,
                ascending: true,
                limit: Some(2),
                offset: Some(1),
                ..Default::default()
            },
        )
        .await
        .expect("page");
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id, "m-2", "offset skips the first row");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn search_covers_latin_cjk_and_arabic() {
        let (pool, path) = migrated_pool("media_repo_search.db").await;
        create(&pool, &sample_media("m-latin", "Sword of the Dawn"))
            .await
            .expect("create");
        create(&pool, &sample_media("m-cjk", "??????"))
            .await
            .expect("create");
        create(&pool, &sample_media("m-arabic", "?????????????"))
            .await
            .expect("create");

        // unicode61 whole-word prefix.
        let hits = search(&pool, "dawn").await.expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "m-latin");

        // trigram substring (3-char window) for CJK.
        let hits = search(&pool, "???").await.expect("search cjk");
        assert!(
            hits.iter().any(|h| h.id == "m-cjk"),
            "CJK substring should match"
        );

        // Arabic query is folded to the index form '??????'.
        let hits = search(&pool, "?????????????").await.expect("search arabic");
        assert!(
            hits.iter().any(|h| h.id == "m-arabic"),
            "voweled Arabic query should match"
        );

        assert!(search(&pool, "  ").await.expect("blank").is_empty());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn personal_tags_resolve_and_create() {
        let (pool, path) = migrated_pool("media_repo_personal_tag.db").await;

        assert!(
            resolve_personal_tag(&pool, "Favorites")
                .await
                .expect("none")
                .is_none(),
            "no personal tag yet"
        );

        create_personal_tag(&pool, "tg-1", "Favorites")
            .await
            .expect("create");
        assert_eq!(
            resolve_personal_tag(&pool, "Favorites")
                .await
                .expect("find"),
            Some("tg-1".to_string())
        );
        assert!(
            resolve_personal_tag(&pool, "favorites")
                .await
                .expect("case")
                .is_none(),
            "personal tags match by exact name"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn add_tag_to_many_links_only_missing_pairs() {
        let (pool, path) = migrated_pool("media_repo_tag_many.db").await;
        create(&pool, &sample_media("m-1", "One"))
            .await
            .expect("create 1");
        create(&pool, &sample_media("m-2", "Two"))
            .await
            .expect("create 2");
        create(&pool, &sample_media("m-3", "Three"))
            .await
            .expect("create 3");
        create_personal_tag(&pool, "tg-1", "Backlog")
            .await
            .expect("tag");

        let ids = vec!["m-1".to_string(), "m-2".to_string(), "m-3".to_string()];
        assert_eq!(
            add_tag_to_many(&pool, "tg-1", &ids)
                .await
                .expect("link all"),
            3
        );

        // Re-running is idempotent: no new links, no duplicate rows.
        let ids = vec!["m-1".to_string(), "m-2".to_string(), "m-3".to_string()];
        assert_eq!(
            add_tag_to_many(&pool, "tg-1", &ids)
                .await
                .expect("link again"),
            0
        );
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_tag")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(n, 3, "no duplicate media_tag rows");

        pool.close().await;
        cleanup_files(&path);
    }
}
