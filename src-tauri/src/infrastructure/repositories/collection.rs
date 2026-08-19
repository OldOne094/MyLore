//! Collection repository (MISSION-019).

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

use super::media::MediaSummary;

/// A collection (or smart-list definition).
#[derive(Debug, Clone)]
pub struct CollectionRecord {
    pub id: String,
    pub name: String,
    pub is_smart: bool,
    pub filter_def: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
}

/// A collection membership entry.
#[derive(Debug, Clone)]
pub struct CollectionMember {
    pub collection_id: String,
    pub media_id: String,
    pub position: i64,
    pub added_at: String,
}

/// Insert a collection.
pub async fn create(pool: &SqlitePool, c: &CollectionRecord) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO collection (id, name, is_smart, filter_def, sort_order, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&c.id)
    .bind(&c.name)
    .bind(c.is_smart)
    .bind(&c.filter_def)
    .bind(c.sort_order)
    .bind(&c.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch one collection (or `None`).
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<CollectionRecord>, AppError> {
    let row = sqlx::query(
        "SELECT id, name, is_smart, filter_def, sort_order, created_at FROM collection WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_collection))
}

/// Update a collection's editable fields.
pub async fn update(pool: &SqlitePool, c: &CollectionRecord) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE collection SET name = ?, is_smart = ?, filter_def = ?, sort_order = ? WHERE id = ?",
    )
    .bind(&c.name)
    .bind(c.is_smart)
    .bind(&c.filter_def)
    .bind(c.sort_order)
    .bind(&c.id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete a collection; members cascade.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM collection WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// List collections ordered by sort_order then name.
pub async fn list(pool: &SqlitePool) -> Result<Vec<CollectionRecord>, AppError> {
    let rows = sqlx::query(
        "SELECT id, name, is_smart, filter_def, sort_order, created_at
         FROM collection ORDER BY sort_order, name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_collection).collect())
}

/// Add a media to a collection.
pub async fn add_member(
    pool: &SqlitePool,
    collection_id: &str,
    media_id: &str,
    position: i64,
    added_at: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO collection_member (collection_id, media_id, position, added_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(collection_id, media_id) DO UPDATE SET position = excluded.position",
    )
    .bind(collection_id)
    .bind(media_id)
    .bind(position)
    .bind(added_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a media from a collection.
pub async fn remove_member(
    pool: &SqlitePool,
    collection_id: &str,
    media_id: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM collection_member WHERE collection_id = ? AND media_id = ?")
        .bind(collection_id)
        .bind(media_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Replace the membership of a collection wholesale, in one transaction.
pub async fn set_members(
    pool: &SqlitePool,
    collection_id: &str,
    members: &[CollectionMember],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM collection_member WHERE collection_id = ?")
        .bind(collection_id)
        .execute(&mut *tx)
        .await?;
    for m in members {
        sqlx::query(
            "INSERT INTO collection_member (collection_id, media_id, position, added_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(&m.collection_id)
        .bind(&m.media_id)
        .bind(m.position)
        .bind(&m.added_at)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Members of a collection, ordered by position.
pub async fn members(
    pool: &SqlitePool,
    collection_id: &str,
) -> Result<Vec<CollectionMember>, AppError> {
    let rows = sqlx::query(
        "SELECT collection_id, media_id, position, added_at
         FROM collection_member WHERE collection_id = ? ORDER BY position, media_id",
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CollectionMember {
            collection_id: row.get(0),
            media_id: row.get(1),
            position: row.get(2),
            added_at: row.get(3),
        })
        .collect())
}

/// Map every media id to its collection names, for the streaming export
/// (MISSION-071). One join query instead of a membership lookup per media.
pub async fn media_collection_names(
    pool: &SqlitePool,
) -> Result<std::collections::HashMap<String, Vec<String>>, AppError> {
    let rows = sqlx::query(
        "SELECT cm.media_id, c.name
         FROM collection_member cm
         JOIN collection c ON c.id = cm.collection_id
         ORDER BY c.sort_order, c.name, cm.media_id",
    )
    .fetch_all(pool)
    .await?;
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for row in rows {
        map.entry(row.get(0)).or_default().push(row.get(1));
    }
    Ok(map)
}

/// List collections with their member counts (MISSION-076), ordered by
/// `sort_order` then name.
pub async fn list_with_counts(pool: &SqlitePool) -> Result<Vec<(CollectionRecord, i64)>, AppError> {
    let rows = sqlx::query(
        "SELECT c.id, c.name, c.is_smart, c.filter_def, c.sort_order, c.created_at,
                COUNT(cm.media_id) AS member_count
         FROM collection c
         LEFT JOIN collection_member cm ON cm.collection_id = c.id
         GROUP BY c.id
         ORDER BY c.sort_order, c.name",
    )
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let count: i64 = row.get(6);
        let collection = row_to_collection(row);
        out.push((collection, count));
    }
    Ok(out)
}

/// Number of members in one collection (0 for an unknown collection).
pub async fn count_members(pool: &SqlitePool, id: &str) -> Result<i64, AppError> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM collection_member WHERE collection_id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

/// A collection's members joined with their media summary rows, ordered by
/// `position` then media id. Drives the collection detail view (MISSION-076).
pub async fn members_with_media(
    pool: &SqlitePool,
    collection_id: &str,
) -> Result<Vec<(CollectionMember, MediaSummary)>, AppError> {
    let rows = sqlx::query(
        "SELECT cm.collection_id, cm.media_id, cm.position, cm.added_at,
                m.id, m.content_type, m.title_main, m.pub_status, m.release_year,
                m.cover_asset_id, COALESCE(r.favorite, 0) AS favorite, m.updated_at
         FROM collection_member cm
         JOIN media m ON m.id = cm.media_id
         LEFT JOIN review r ON r.media_id = m.id
         WHERE cm.collection_id = ?
         ORDER BY cm.position, cm.media_id",
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push((
            CollectionMember {
                collection_id: row.get(0),
                media_id: row.get(1),
                position: row.get(2),
                added_at: row.get(3),
            },
            MediaSummary {
                id: row.get(4),
                content_type: row.get(5),
                title_main: row.get(6),
                pub_status: row.get(7),
                release_year: row.get(8),
                cover_asset_id: row.get(9),
                favorite: row.get::<i64, _>(10) != 0,
                updated_at: row.get(11),
            },
        ));
    }
    Ok(out)
}

fn row_to_collection(row: SqliteRow) -> CollectionRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    CollectionRecord {
        id: get(0).expect("id"),
        name: get(1).expect("name"),
        is_smart: row.get::<i64, _>(2) != 0,
        filter_def: get(3),
        sort_order: row.get(4),
        created_at: get(5).expect("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn collection(id: &str) -> CollectionRecord {
        CollectionRecord {
            id: id.to_string(),
            name: format!("List {id}"),
            is_smart: false,
            filter_def: None,
            sort_order: 0,
            created_at: "2026-01-01".to_string(),
        }
    }

    async fn seed_media(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, 'novel', 'Title', '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("seed media");
    }

    #[tokio::test]
    async fn media_collection_names_groups_by_media() {
        let (pool, path) = migrated_pool("collection_repo_export.db").await;
        create(&pool, &collection("c-1")).await.expect("create c1");
        create(&pool, &collection("c-2")).await.expect("create c2");
        for media in ["m-1", "m-2"] {
            seed_media(&pool, media).await;
        }
        add_member(&pool, "c-1", "m-1", 0, "2026-01-01")
            .await
            .expect("m1 in c1");
        add_member(&pool, "c-2", "m-1", 1, "2026-01-01")
            .await
            .expect("m1 in c2");
        add_member(&pool, "c-2", "m-2", 0, "2026-01-01")
            .await
            .expect("m2 in c2");

        let map = media_collection_names(&pool).await.expect("map");
        assert_eq!(
            map.get("m-1").unwrap(),
            &vec!["List c-1".to_string(), "List c-2".to_string()]
        );
        assert_eq!(map.get("m-2").unwrap(), &vec!["List c-2".to_string()]);
        assert!(!map.contains_key("missing"));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn members_add_remove_and_replace() {
        let (pool, path) = migrated_pool("collection_repo.db").await;
        create(&pool, &collection("c-1")).await.expect("create");
        seed_media(&pool, "m-1").await;
        seed_media(&pool, "m-2").await;
        seed_media(&pool, "m-3").await;

        add_member(&pool, "c-1", "m-2", 1, "2026-01-01")
            .await
            .expect("add m2");
        add_member(&pool, "c-1", "m-1", 0, "2026-01-01")
            .await
            .expect("add m1");

        let current = members(&pool, "c-1").await.expect("members");
        assert_eq!(current.len(), 2);
        assert_eq!(current[0].media_id, "m-1", "ordered by position");

        remove_member(&pool, "c-1", "m-1").await.expect("remove");
        let current = members(&pool, "c-1").await.expect("members");
        assert_eq!(current.len(), 1);

        let replacement = vec![
            CollectionMember {
                collection_id: "c-1".into(),
                media_id: "m-3".into(),
                position: 0,
                added_at: "2026-02-01".into(),
            },
            CollectionMember {
                collection_id: "c-1".into(),
                media_id: "m-1".into(),
                position: 1,
                added_at: "2026-02-01".into(),
            },
        ];
        set_members(&pool, "c-1", &replacement)
            .await
            .expect("replace");
        let current = members(&pool, "c-1").await.expect("members");
        assert_eq!(current.len(), 2);
        assert_eq!(current[0].media_id, "m-3");

        let listed = list(&pool).await.expect("list");
        assert_eq!(listed.len(), 1);

        delete(&pool, "c-1").await.expect("delete");
        let current = members(&pool, "c-1").await.expect("members");
        assert!(current.is_empty(), "members cascade with the collection");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn update_changes_name_and_smart_flags() {
        let (pool, path) = migrated_pool("collection_repo_update.db").await;
        create(&pool, &collection("c-1")).await.expect("create");

        let mut c = collection("c-1");
        c.name = "Reading Now".into();
        c.is_smart = true;
        c.filter_def = Some("{\"status\":\"in_progress\"}".into());
        update(&pool, &c).await.expect("update");

        let got = get(&pool, "c-1").await.expect("get").unwrap();
        assert_eq!(got.name, "Reading Now");
        assert!(got.is_smart);
        assert_eq!(
            got.filter_def.as_deref(),
            Some("{\"status\":\"in_progress\"}")
        );
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn list_with_counts_orders_and_counts_members() {
        let (pool, path) = migrated_pool("collection_repo_counts.db").await;
        create(&pool, &collection("c-2")).await.expect("create c2");
        create(&pool, &collection("c-1")).await.expect("create c1");
        seed_media(&pool, "m-1").await;
        seed_media(&pool, "m-2").await;
        add_member(&pool, "c-2", "m-1", 0, "2026-01-01")
            .await
            .expect("m1 in c2");

        let rows = list_with_counts(&pool).await.expect("list with counts");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0.id, "c-1", "ordered by name");
        assert_eq!(rows[0].1, 0, "empty collection counts zero");
        assert_eq!(rows[1].0.id, "c-2");
        assert_eq!(rows[1].1, 1, "one member");

        assert_eq!(
            count_members(&pool, "c-2").await.expect("count c2"),
            1,
            "count_members matches"
        );
        assert_eq!(
            count_members(&pool, "c-missing")
                .await
                .expect("count missing"),
            0
        );
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn members_with_media_joins_rows_in_position_order() {
        let (pool, path) = migrated_pool("collection_repo_join.db").await;
        create(&pool, &collection("c-1")).await.expect("create");
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'anime', 'One', '2026-01-01', '2026-01-01'),
                    ('m-2', 'novel', 'Two', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        sqlx::query(
            "INSERT INTO review (media_id, rating, review, favorite, is_spoiler, created_at, updated_at)
             VALUES ('m-2', 8, 'nice', 1, 0, '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed review");
        add_member(&pool, "c-1", "m-2", 1, "2026-01-02")
            .await
            .expect("m2");
        add_member(&pool, "c-1", "m-1", 0, "2026-01-01")
            .await
            .expect("m1");

        let joined = members_with_media(&pool, "c-1")
            .await
            .expect("joined members");
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[0].0.media_id, "m-1", "position order");
        assert_eq!(joined[0].1.title_main, "One");
        assert!(!joined[0].1.favorite);
        assert_eq!(joined[1].0.media_id, "m-2");
        assert!(joined[1].1.favorite, "favorite rides from the review row");
        assert_eq!(joined[1].1.content_type, "novel");

        assert!(members_with_media(&pool, "c-missing")
            .await
            .expect("empty")
            .is_empty());
        pool.close().await;
        cleanup_files(&path);
    }
}
