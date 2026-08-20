//! Year-in-review repository (MISSION-082).
//!
//! One helper feeds the recap service: the top genres of the media completed
//! in a year. The activity rows themselves reuse `calendar::activity_in_range`
//! — the recap needs exactly the same projection (media_id, title,
//! content_type, kind, created_at).

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

/// A genre ranked by how many distinct completed media carry it.
#[derive(Debug, Clone)]
pub struct GenreCountRow {
    pub name: String,
    pub count: u32,
}

/// Top genres of the given media, ranked by distinct-media count.
pub async fn completed_genres(
    pool: &SqlitePool,
    media_ids: &[String],
) -> Result<Vec<GenreCountRow>, AppError> {
    if media_ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = media_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT g.name, COUNT(DISTINCT mg.media_id) AS cnt \
         FROM media_genre mg JOIN genre g ON g.id = mg.genre_id \
         WHERE mg.media_id IN ({placeholders}) \
         GROUP BY g.name ORDER BY cnt DESC, g.name LIMIT 5"
    );
    let mut query = sqlx::query(&sql);
    for id in media_ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(pool).await?;
    Ok(rows.into_iter().map(row_to_genre).collect())
}

fn row_to_genre(row: SqliteRow) -> GenreCountRow {
    GenreCountRow {
        name: row.get("name"),
        count: row.get::<i64, _>("cnt") as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn seed_media(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, 'anime', 'Series', '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("seed media");
    }

    async fn link_genre(pool: &SqlitePool, media_id: &str, genre: &str) {
        sqlx::query("INSERT INTO media_genre (media_id, genre_id) VALUES (?, ?)")
            .bind(media_id)
            .bind(genre)
            .execute(pool)
            .await
            .expect("link genre");
    }

    #[tokio::test]
    async fn ranks_genres_by_distinct_completed_media() {
        let (pool, path) = migrated_pool("recap_genres.db").await;
        seed_media(&pool, "m-1").await;
        seed_media(&pool, "m-2").await;
        seed_media(&pool, "m-3").await;
        link_genre(&pool, "m-1", "fantasy").await;
        link_genre(&pool, "m-1", "adventure").await;
        link_genre(&pool, "m-2", "fantasy").await;
        link_genre(&pool, "m-2", "romance").await;
        link_genre(&pool, "m-3", "action").await;

        let rows = completed_genres(&pool, &["m-1".into(), "m-2".into()])
            .await
            .expect("genres");
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Fantasy", "Adventure", "Romance"]);
        assert_eq!(rows[0].count, 2, "fantasy appears on two distinct media");
        assert_eq!(rows[1].count, 1);

        let none = completed_genres(&pool, &[]).await.expect("empty set");
        assert!(none.is_empty());

        pool.close().await;
        cleanup_files(&path);
    }
}
