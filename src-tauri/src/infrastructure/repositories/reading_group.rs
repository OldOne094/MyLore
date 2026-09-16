//! Reading-group repository (MISSION-114).
//!
//! SQL for the reading-group aggregate: `reading_group` + `group_member` +
//! `group_shelf` + `group_note`. Repositories stay clock-free — timestamps are
//! supplied by the caller — and carry no domain logic beyond the columns.

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

/// A reading group row.
#[derive(Debug, Clone)]
pub struct GroupRecord {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub epoch: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// A group membership row.
#[derive(Debug, Clone)]
pub struct MemberRecord {
    pub group_id: String,
    pub member_id: String,
    pub display_name: String,
    /// `owner` | `member` (the table's CHECK).
    pub role: String,
    pub joined_at: String,
}

/// One member's shelf entry for a work.
#[derive(Debug, Clone)]
pub struct ShelfRecord {
    pub group_id: String,
    pub member_id: String,
    pub work_key: String,
    pub title: String,
    pub content_type: String,
    /// A `CoreStatus` storage string.
    pub status: String,
    pub progress: i64,
    pub updated_at: String,
}

/// A shared note on a work.
#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub id: String,
    pub group_id: String,
    pub work_key: String,
    pub author_id: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------- groups

pub async fn create_group<'e, E>(executor: E, g: &GroupRecord) -> Result<(), AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO reading_group (id, name, owner_id, epoch, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&g.id)
    .bind(&g.name)
    .bind(&g.owner_id)
    .bind(g.epoch)
    .bind(&g.created_at)
    .bind(&g.updated_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn get_group(pool: &SqlitePool, id: &str) -> Result<Option<GroupRecord>, AppError> {
    let row = sqlx::query(
        "SELECT id, name, owner_id, epoch, created_at, updated_at FROM reading_group WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_group))
}

/// List groups newest first.
pub async fn list_groups(pool: &SqlitePool) -> Result<Vec<GroupRecord>, AppError> {
    let rows = sqlx::query(
        "SELECT id, name, owner_id, epoch, created_at, updated_at
         FROM reading_group ORDER BY created_at DESC, name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_group).collect())
}

pub async fn rename_group(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    updated_at: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE reading_group SET name = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(updated_at)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a group; members, shelf rows and notes cascade.
pub async fn delete_group(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM reading_group WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Increment the group's epoch (the member-removal seam; key rotation in
/// MISSION-118 consumes it). Returns the new epoch value.
pub async fn bump_epoch<'e, E>(executor: E, id: &str, updated_at: &str) -> Result<i64, AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let (epoch,): (i64,) = sqlx::query_as(
        "UPDATE reading_group SET epoch = epoch + 1, updated_at = ? WHERE id = ? RETURNING epoch",
    )
    .bind(updated_at)
    .bind(id)
    .fetch_one(executor)
    .await?;
    Ok(epoch)
}

// --------------------------------------------------------------- members

pub async fn list_members(
    pool: &SqlitePool,
    group_id: &str,
) -> Result<Vec<MemberRecord>, AppError> {
    let rows = sqlx::query(
        "SELECT group_id, member_id, display_name, role, joined_at FROM group_member
         WHERE group_id = ? ORDER BY role, display_name, member_id",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_member).collect())
}

pub async fn get_member(
    pool: &SqlitePool,
    group_id: &str,
    member_id: &str,
) -> Result<Option<MemberRecord>, AppError> {
    let row = sqlx::query(
        "SELECT group_id, member_id, display_name, role, joined_at FROM group_member
         WHERE group_id = ? AND member_id = ?",
    )
    .bind(group_id)
    .bind(member_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_member))
}

pub async fn add_member<'e, E>(executor: E, m: &MemberRecord) -> Result<(), AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO group_member (group_id, member_id, display_name, role, joined_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(group_id, member_id) DO UPDATE SET
           display_name = excluded.display_name,
           role = excluded.role",
    )
    .bind(&m.group_id)
    .bind(&m.member_id)
    .bind(&m.display_name)
    .bind(&m.role)
    .bind(&m.joined_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn remove_member<'e, E>(
    executor: E,
    group_id: &str,
    member_id: &str,
) -> Result<(), AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("DELETE FROM group_member WHERE group_id = ? AND member_id = ?")
        .bind(group_id)
        .bind(member_id)
        .execute(executor)
        .await?;
    Ok(())
}

// ----------------------------------------------------------------- shelf

/// One group's shelf rows, optionally filtered to a single member.
pub async fn list_shelf(
    pool: &SqlitePool,
    group_id: &str,
    member_id: Option<&str>,
) -> Result<Vec<ShelfRecord>, AppError> {
    let rows = match member_id {
        Some(member) => {
            sqlx::query(
                "SELECT group_id, member_id, work_key, title, content_type, status, progress,
                        updated_at
                 FROM group_shelf WHERE group_id = ? AND member_id = ?
                 ORDER BY title COLLATE NOCASE, work_key",
            )
            .bind(group_id)
            .bind(member)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT group_id, member_id, work_key, title, content_type, status, progress,
                        updated_at
                 FROM group_shelf WHERE group_id = ?
                 ORDER BY work_key, member_id",
            )
            .bind(group_id)
            .fetch_all(pool)
            .await?
        }
    };
    Ok(rows.into_iter().map(row_to_shelf).collect())
}

/// Insert or overwrite one member's shelf entry for a work (single-writer).
pub async fn upsert_shelf(pool: &SqlitePool, s: &ShelfRecord) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO group_shelf
           (group_id, member_id, work_key, title, content_type, status, progress, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(group_id, member_id, work_key) DO UPDATE SET
           title = excluded.title,
           content_type = excluded.content_type,
           status = excluded.status,
           progress = excluded.progress,
           updated_at = excluded.updated_at",
    )
    .bind(&s.group_id)
    .bind(&s.member_id)
    .bind(&s.work_key)
    .bind(&s.title)
    .bind(&s.content_type)
    .bind(&s.status)
    .bind(s.progress)
    .bind(&s.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

// ----------------------------------------------------------------- notes

/// Notes for a group (all works, or one work when `work_key` is given),
/// oldest first so threads read top-down.
pub async fn list_notes(
    pool: &SqlitePool,
    group_id: &str,
    work_key: Option<&str>,
) -> Result<Vec<NoteRecord>, AppError> {
    let rows = match work_key {
        Some(key) => {
            sqlx::query(
                "SELECT id, group_id, work_key, author_id, body, created_at, updated_at
                 FROM group_note WHERE group_id = ? AND work_key = ? ORDER BY created_at, id",
            )
            .bind(group_id)
            .bind(key)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, group_id, work_key, author_id, body, created_at, updated_at
                 FROM group_note WHERE group_id = ? ORDER BY created_at, id",
            )
            .bind(group_id)
            .fetch_all(pool)
            .await?
        }
    };
    Ok(rows.into_iter().map(row_to_note).collect())
}

pub async fn get_note(pool: &SqlitePool, id: &str) -> Result<Option<NoteRecord>, AppError> {
    let row = sqlx::query(
        "SELECT id, group_id, work_key, author_id, body, created_at, updated_at
         FROM group_note WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_note))
}

pub async fn insert_note(pool: &SqlitePool, n: &NoteRecord) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO group_note (id, group_id, work_key, author_id, body, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&n.id)
    .bind(&n.group_id)
    .bind(&n.work_key)
    .bind(&n.author_id)
    .bind(&n.body)
    .bind(&n.created_at)
    .bind(&n.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update a note body; returns whether a row was changed.
pub async fn update_note(
    pool: &SqlitePool,
    id: &str,
    body: &str,
    updated_at: &str,
) -> Result<bool, AppError> {
    let result = sqlx::query("UPDATE group_note SET body = ?, updated_at = ? WHERE id = ?")
        .bind(body)
        .bind(updated_at)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_note(pool: &SqlitePool, id: &str) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM group_note WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// --------------------------------------------------------------- mapping

fn row_to_group(row: SqliteRow) -> GroupRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    GroupRecord {
        id: get(0).expect("id"),
        name: get(1).expect("name"),
        owner_id: get(2).expect("owner_id"),
        epoch: row.get(3),
        created_at: get(4).expect("created_at"),
        updated_at: get(5).expect("updated_at"),
    }
}

fn row_to_member(row: SqliteRow) -> MemberRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    MemberRecord {
        group_id: get(0).expect("group_id"),
        member_id: get(1).expect("member_id"),
        display_name: get(2).expect("display_name"),
        role: get(3).expect("role"),
        joined_at: get(4).expect("joined_at"),
    }
}

fn row_to_shelf(row: SqliteRow) -> ShelfRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    ShelfRecord {
        group_id: get(0).expect("group_id"),
        member_id: get(1).expect("member_id"),
        work_key: get(2).expect("work_key"),
        title: get(3).expect("title"),
        content_type: get(4).expect("content_type"),
        status: get(5).expect("status"),
        progress: row.get(6),
        updated_at: get(7).expect("updated_at"),
    }
}

fn row_to_note(row: SqliteRow) -> NoteRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    NoteRecord {
        id: get(0).expect("id"),
        group_id: get(1).expect("group_id"),
        work_key: get(2).expect("work_key"),
        author_id: get(3).expect("author_id"),
        body: get(4).expect("body"),
        created_at: get(5).expect("created_at"),
        updated_at: get(6).expect("updated_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn group(id: &str) -> GroupRecord {
        GroupRecord {
            id: id.into(),
            name: "Book Club".into(),
            owner_id: "m-local".into(),
            epoch: 0,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        }
    }

    fn member(group_id: &str, id: &str, role: &str) -> MemberRecord {
        MemberRecord {
            group_id: group_id.into(),
            member_id: id.into(),
            display_name: format!("Member {id}"),
            role: role.into(),
            joined_at: "2026-01-01".into(),
        }
    }

    #[tokio::test]
    async fn group_member_shelf_and_note_roundtrip() {
        let (pool, path) = migrated_pool("rg_repo_roundtrip.db").await;
        create_group(&pool, &group("g-1")).await.expect("create");

        let fetched = get_group(&pool, "g-1").await.expect("get").unwrap();
        assert_eq!(fetched.name, "Book Club");
        assert_eq!(fetched.epoch, 0);

        add_member(&pool, &member("g-1", "m-1", "owner"))
            .await
            .expect("member");
        assert!(get_member(&pool, "g-1", "m-1").await.unwrap().is_some());
        assert!(get_member(&pool, "g-1", "m-2").await.unwrap().is_none());

        upsert_shelf(
            &pool,
            &ShelfRecord {
                group_id: "g-1".into(),
                member_id: "m-1".into(),
                work_key: "anilist:42".into(),
                title: "Berserk".into(),
                content_type: "manga".into(),
                status: "in_progress".into(),
                progress: 3,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("shelf");
        // Upsert overwrites the same (group, member, work).
        upsert_shelf(
            &pool,
            &ShelfRecord {
                group_id: "g-1".into(),
                member_id: "m-1".into(),
                work_key: "anilist:42".into(),
                title: "Berserk".into(),
                content_type: "manga".into(),
                status: "completed".into(),
                progress: 41,
                updated_at: "2026-01-03".into(),
            },
        )
        .await
        .expect("re-shelf");
        let shelf = list_shelf(&pool, "g-1", Some("m-1")).await.expect("list");
        assert_eq!(shelf.len(), 1);
        assert_eq!(shelf[0].status, "completed");
        assert_eq!(shelf[0].progress, 41);

        insert_note(
            &pool,
            &NoteRecord {
                id: "n-1".into(),
                group_id: "g-1".into(),
                work_key: "anilist:42".into(),
                author_id: "m-1".into(),
                body: "Great".into(),
                created_at: "2026-01-02".into(),
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("note");
        assert_eq!(
            list_notes(&pool, "g-1", Some("anilist:42"))
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(update_note(&pool, "n-1", "Even better", "2026-01-04")
            .await
            .unwrap());
        assert_eq!(
            get_note(&pool, "n-1").await.unwrap().unwrap().body,
            "Even better"
        );
        assert!(delete_note(&pool, "n-1").await.unwrap());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn deleting_a_group_cascades_members_shelf_and_notes() {
        let (pool, path) = migrated_pool("rg_repo_cascade.db").await;
        create_group(&pool, &group("g-1")).await.expect("create");
        add_member(&pool, &member("g-1", "m-1", "owner"))
            .await
            .expect("member");
        upsert_shelf(
            &pool,
            &ShelfRecord {
                group_id: "g-1".into(),
                member_id: "m-1".into(),
                work_key: "anilist:1".into(),
                title: "T".into(),
                content_type: "manga".into(),
                status: "planned".into(),
                progress: 0,
                updated_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("shelf");
        insert_note(
            &pool,
            &NoteRecord {
                id: "n-1".into(),
                group_id: "g-1".into(),
                work_key: "anilist:1".into(),
                author_id: "m-1".into(),
                body: "hi".into(),
                created_at: "2026-01-01".into(),
                updated_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("note");

        delete_group(&pool, "g-1").await.expect("delete");
        assert!(list_members(&pool, "g-1").await.unwrap().is_empty());
        assert!(list_shelf(&pool, "g-1", None).await.unwrap().is_empty());
        assert!(list_notes(&pool, "g-1", None).await.unwrap().is_empty());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn bump_epoch_increments_and_returns_the_new_value() {
        let (pool, path) = migrated_pool("rg_repo_epoch.db").await;
        create_group(&pool, &group("g-1")).await.expect("create");
        assert_eq!(bump_epoch(&pool, "g-1", "2026-01-02").await.unwrap(), 1);
        assert_eq!(bump_epoch(&pool, "g-1", "2026-01-03").await.unwrap(), 2);
        pool.close().await;
        cleanup_files(&path);
    }
}
