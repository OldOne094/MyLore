//! Reading-group use-cases (MISSION-114).
//!
//! Local, offline-only CRUD over the reading-group aggregate plus the manual
//! `group_state.json` export/import that acts as the first "transport". Group
//! state never touches the personal aggregates (ADR-007); works are referenced
//! by the stable [`crate::domain::reading_group::work_key`].
//!
//! Everything here stays on-device. The opt-in flag (`readingGroup.enabled`) is
//! read by the future UI (MISSION-117); this mission ships no network.

use std::path::Path;
use std::str::FromStr;

use chrono::Utc;
use sqlx::SqlitePool;
use tracing::info;
use uuid::Uuid;

use crate::domain::enums::CoreStatus;
use crate::domain::reading_group::{Group, GroupMember, GroupNote, MemberRole, ShelfEntry};
use crate::error::AppError;
use crate::infrastructure::repositories::reading_group as repo;
use crate::infrastructure::repositories::reading_group::{
    GroupRecord, MemberRecord, NoteRecord, ShelfRecord,
};

const KEY_ENABLED: &str = "readingGroup.enabled";
const KEY_MEMBER_ID: &str = "readingGroup.memberId";
const KEY_DISPLAY_NAME: &str = "readingGroup.displayName";

const STATE_FORMAT: &str = "mylore.reading-group-state";
const STATE_VERSION: u32 = 1;

/// Local, opt-in reading-group identity/preferences.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupPrefs {
    /// The feature is off by default; the UI hides group surfaces until on.
    pub enabled: bool,
    /// This install's stable member id (`m-<uuid>`).
    pub member_id: String,
    /// Display name other members see.
    pub display_name: String,
}

/// A group membership row for the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupMemberView {
    pub member_id: String,
    pub display_name: String,
    pub role: String,
    pub joined_at: String,
}

/// One shelf row.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupShelfEntryView {
    pub group_id: String,
    pub member_id: String,
    pub work_key: String,
    pub title: String,
    pub content_type: String,
    pub status: String,
    pub progress: i64,
    pub updated_at: String,
}

/// A shared note row.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupNoteView {
    pub id: String,
    pub group_id: String,
    pub work_key: String,
    pub author_id: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A group with its members and row counts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupView {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub epoch: i64,
    pub created_at: String,
    pub updated_at: String,
    pub members: Vec<GroupMemberView>,
    pub shelf_count: usize,
    pub note_count: usize,
}

/// Outcome of writing a `group_state.json` file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupExportReport {
    pub path: String,
    pub groups: usize,
    pub members: usize,
    pub shelf: usize,
    pub notes: usize,
}

/// Outcome of importing a `group_state.json` payload.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupImportReport {
    pub groups: usize,
    pub members: usize,
    pub shelf: usize,
    pub notes: usize,
    /// Rows left untouched because the local copy was equal-or-newer.
    pub skipped: usize,
}

// ------------------------------------------------------- exported file model

/// The `group_state.json` document (format-tagged + versioned).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupState {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub groups: Vec<GroupStateEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupStateEntry {
    pub group: GroupStateGroup,
    pub members: Vec<GroupStateMember>,
    pub shelf: Vec<GroupStateShelf>,
    pub notes: Vec<GroupStateNote>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupStateGroup {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub epoch: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupStateMember {
    pub member_id: String,
    pub display_name: String,
    pub role: String,
    pub joined_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupStateShelf {
    pub member_id: String,
    pub work_key: String,
    pub title: String,
    pub content_type: String,
    pub status: String,
    pub progress: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupStateNote {
    pub id: String,
    pub work_key: String,
    pub author_id: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Reading-group use-cases.
pub struct ReadingGroupService {
    pool: SqlitePool,
}

impl ReadingGroupService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ------------------------------------------------------------- prefs

    /// Read the local opt-in flag + identity. A missing member id is minted and
    /// persisted so the identity is stable across sessions.
    pub async fn prefs(&self) -> Result<GroupPrefs, AppError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM settings WHERE key IN (?, ?, ?)")
                .bind(KEY_ENABLED)
                .bind(KEY_MEMBER_ID)
                .bind(KEY_DISPLAY_NAME)
                .fetch_all(&self.pool)
                .await?;
        let mut enabled = false;
        let mut member_id = String::new();
        let mut display_name = String::new();
        for (key, value) in rows {
            match key.as_str() {
                KEY_ENABLED => enabled = value == "true",
                KEY_MEMBER_ID => member_id = value,
                KEY_DISPLAY_NAME => display_name = value,
                _ => {}
            }
        }
        if member_id.trim().is_empty() {
            member_id = format!("m-{}", Uuid::new_v4());
            self.put_setting(KEY_MEMBER_ID, &member_id).await?;
        }
        Ok(GroupPrefs {
            enabled,
            member_id,
            display_name,
        })
    }

    /// Persist the opt-in flag / display name (the member id is immutable).
    pub async fn set_prefs(
        &self,
        enabled: bool,
        display_name: &str,
    ) -> Result<GroupPrefs, AppError> {
        let name = display_name.trim();
        self.put_setting(KEY_ENABLED, if enabled { "true" } else { "false" })
            .await?;
        self.put_setting(KEY_DISPLAY_NAME, name).await?;
        // Ensure a member id exists even when enabling before first read.
        let prefs = self.prefs().await?;
        Ok(GroupPrefs {
            enabled,
            display_name: name.to_string(),
            ..prefs
        })
    }

    async fn put_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ------------------------------------------------------------ groups

    /// Create a group and seed its owner membership from the local identity.
    pub async fn create_group(&self, name: &str) -> Result<GroupView, AppError> {
        let group = Group {
            id: format!("g-{}", Uuid::new_v4()),
            name: name.trim().to_string(),
            owner_id: self.prefs().await?.member_id,
            epoch: 0,
            created_at: now(),
            updated_at: now(),
        };
        group.validate().map_err(AppError::from)?;
        info!(group = %group.id, "reading_group_create");

        let prefs = self.prefs().await?;
        let owner = GroupMember {
            group_id: group.id.clone(),
            member_id: group.owner_id.clone(),
            display_name: if prefs.display_name.trim().is_empty() {
                "Me".to_string()
            } else {
                prefs.display_name.clone()
            },
            role: MemberRole::Owner,
            joined_at: group.created_at.clone(),
        };
        owner.validate().map_err(AppError::from)?;

        let mut tx = self.pool.begin().await?;
        repo::create_group(
            &mut *tx,
            &GroupRecord {
                id: group.id.clone(),
                name: group.name.clone(),
                owner_id: group.owner_id.clone(),
                epoch: group.epoch,
                created_at: group.created_at.clone(),
                updated_at: group.updated_at.clone(),
            },
        )
        .await?;
        repo::add_member(
            &mut *tx,
            &MemberRecord {
                group_id: owner.group_id,
                member_id: owner.member_id,
                display_name: owner.display_name,
                role: owner.role.as_str().to_string(),
                joined_at: owner.joined_at,
            },
        )
        .await?;
        tx.commit().await?;

        self.view_group(&group.id).await
    }

    pub async fn rename_group(&self, group_id: &str, name: &str) -> Result<GroupView, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::validation("group name must not be empty"));
        }
        self.require_group(group_id).await?;
        repo::rename_group(&self.pool, group_id, name, &now()).await?;
        self.view_group(group_id).await
    }

    pub async fn delete_group(&self, group_id: &str) -> Result<(), AppError> {
        self.require_group(group_id).await?;
        repo::delete_group(&self.pool, group_id).await
    }

    pub async fn list_groups(&self) -> Result<Vec<GroupView>, AppError> {
        let mut views = Vec::new();
        for group in repo::list_groups(&self.pool).await? {
            views.push(self.view_of(&group).await?);
        }
        Ok(views)
    }

    pub async fn view_group(&self, group_id: &str) -> Result<GroupView, AppError> {
        let group = self.require_group(group_id).await?;
        self.view_of(&group).await
    }

    async fn view_of(&self, group: &GroupRecord) -> Result<GroupView, AppError> {
        let members = repo::list_members(&self.pool, &group.id).await?;
        let shelf_count = repo::list_shelf(&self.pool, &group.id, None).await?.len();
        let note_count = repo::list_notes(&self.pool, &group.id, None).await?.len();
        Ok(GroupView {
            id: group.id.clone(),
            name: group.name.clone(),
            owner_id: group.owner_id.clone(),
            epoch: group.epoch,
            created_at: group.created_at.clone(),
            updated_at: group.updated_at.clone(),
            members: members
                .into_iter()
                .map(|m| GroupMemberView {
                    member_id: m.member_id,
                    display_name: m.display_name,
                    role: m.role,
                    joined_at: m.joined_at,
                })
                .collect(),
            shelf_count,
            note_count,
        })
    }

    async fn require_group(&self, group_id: &str) -> Result<GroupRecord, AppError> {
        repo::get_group(&self.pool, group_id)
            .await?
            .ok_or_else(|| AppError::validation(format!("unknown group: {group_id}")))
    }

    // ----------------------------------------------------------- members

    pub async fn add_member(
        &self,
        group_id: &str,
        member_id: &str,
        display_name: &str,
        role: &str,
    ) -> Result<GroupView, AppError> {
        self.require_group(group_id).await?;
        let member = GroupMember {
            group_id: group_id.to_string(),
            member_id: member_id.trim().to_string(),
            display_name: display_name.trim().to_string(),
            role: MemberRole::from_str(role).map_err(AppError::from)?,
            joined_at: now(),
        };
        member.validate().map_err(AppError::from)?;
        repo::add_member(
            &self.pool,
            &MemberRecord {
                group_id: member.group_id,
                member_id: member.member_id,
                display_name: member.display_name,
                role: member.role.as_str().to_string(),
                joined_at: member.joined_at,
            },
        )
        .await?;
        self.view_group(group_id).await
    }

    /// Remove a member. The owner can never be removed; the group epoch is
    /// bumped so the future key rotation (MISSION-118) has a stable counter.
    pub async fn remove_member(
        &self,
        group_id: &str,
        member_id: &str,
    ) -> Result<GroupView, AppError> {
        let group = self.require_group(group_id).await?;
        if member_id == group.owner_id {
            return Err(AppError::validation("the group owner cannot be removed"));
        }
        let Some(member) = repo::get_member(&self.pool, group_id, member_id).await? else {
            return Err(AppError::validation(format!("unknown member: {member_id}")));
        };
        if member.role == MemberRole::Owner.as_str() {
            return Err(AppError::validation("the group owner cannot be removed"));
        }
        let mut tx = self.pool.begin().await?;
        repo::remove_member(&mut *tx, group_id, member_id).await?;
        repo::bump_epoch(&mut *tx, group_id, &now()).await?;
        tx.commit().await?;
        self.view_group(group_id).await
    }

    // ------------------------------------------------------------- shelf

    pub async fn shelf(
        &self,
        group_id: &str,
        member_id: Option<&str>,
    ) -> Result<Vec<GroupShelfEntryView>, AppError> {
        self.require_group(group_id).await?;
        Ok(repo::list_shelf(&self.pool, group_id, member_id)
            .await?
            .into_iter()
            .map(shelf_view)
            .collect())
    }

    /// Upsert one member's shelf entry (single-writer per member).
    #[allow(clippy::too_many_arguments)]
    pub async fn set_shelf_entry(
        &self,
        group_id: &str,
        member_id: &str,
        work_key: &str,
        title: &str,
        content_type: &str,
        status: &str,
        progress: i64,
    ) -> Result<GroupShelfEntryView, AppError> {
        self.require_group(group_id).await?;
        if repo::get_member(&self.pool, group_id, member_id)
            .await?
            .is_none()
        {
            return Err(AppError::validation(format!("unknown member: {member_id}")));
        }
        let entry = ShelfEntry {
            group_id: group_id.to_string(),
            member_id: member_id.to_string(),
            work_key: work_key.trim().to_string(),
            title: title.trim().to_string(),
            content_type: content_type.trim().to_string(),
            status: CoreStatus::from_str(status).map_err(AppError::from)?,
            progress,
            updated_at: now(),
        };
        entry.validate().map_err(AppError::from)?;
        let record = ShelfRecord {
            group_id: entry.group_id,
            member_id: entry.member_id,
            work_key: entry.work_key,
            title: entry.title,
            content_type: entry.content_type,
            status: entry.status.as_str().to_string(),
            progress: entry.progress,
            updated_at: entry.updated_at,
        };
        repo::upsert_shelf(&self.pool, &record).await?;
        Ok(shelf_view(record))
    }

    // ------------------------------------------------------------- notes

    pub async fn notes(
        &self,
        group_id: &str,
        work_key: Option<&str>,
    ) -> Result<Vec<GroupNoteView>, AppError> {
        self.require_group(group_id).await?;
        Ok(repo::list_notes(&self.pool, group_id, work_key)
            .await?
            .into_iter()
            .map(note_view)
            .collect())
    }

    pub async fn add_note(
        &self,
        group_id: &str,
        work_key: &str,
        author_id: &str,
        body: &str,
    ) -> Result<GroupNoteView, AppError> {
        self.require_group(group_id).await?;
        if repo::get_member(&self.pool, group_id, author_id)
            .await?
            .is_none()
        {
            return Err(AppError::validation(format!("unknown member: {author_id}")));
        }
        let note = GroupNote {
            id: format!("n-{}", Uuid::new_v4()),
            group_id: group_id.to_string(),
            work_key: work_key.trim().to_string(),
            author_id: author_id.to_string(),
            body: body.trim().to_string(),
            created_at: now(),
            updated_at: now(),
        };
        note.validate().map_err(AppError::from)?;
        repo::insert_note(&self.pool, &note_record(&note)).await?;
        Ok(note_view(note_record(&note)))
    }

    pub async fn update_note(&self, note_id: &str, body: &str) -> Result<GroupNoteView, AppError> {
        let note = repo::get_note(&self.pool, note_id)
            .await?
            .ok_or_else(|| AppError::validation(format!("unknown note: {note_id}")))?;
        let body = body.trim();
        if body.is_empty() {
            return Err(AppError::validation("note body must not be empty"));
        }
        repo::update_note(&self.pool, note_id, body, &now()).await?;
        let updated = repo::get_note(&self.pool, note_id).await?.unwrap_or(note);
        Ok(note_view(updated))
    }

    pub async fn delete_note(&self, note_id: &str) -> Result<(), AppError> {
        if !repo::delete_note(&self.pool, note_id).await? {
            return Err(AppError::validation(format!("unknown note: {note_id}")));
        }
        Ok(())
    }

    // ---------------------------------------------------- export / import

    /// Collect one group (`Some`) or every group (`None`) into the file model.
    pub async fn export_state(&self, group_id: Option<&str>) -> Result<GroupState, AppError> {
        let groups = match group_id {
            Some(id) => vec![self.require_group(id).await?],
            None => repo::list_groups(&self.pool).await?,
        };
        let mut entries = Vec::new();
        for group in groups {
            let members = repo::list_members(&self.pool, &group.id).await?;
            let shelf = repo::list_shelf(&self.pool, &group.id, None).await?;
            let notes = repo::list_notes(&self.pool, &group.id, None).await?;
            entries.push(GroupStateEntry {
                group: GroupStateGroup {
                    id: group.id,
                    name: group.name,
                    owner_id: group.owner_id,
                    epoch: group.epoch,
                    created_at: group.created_at,
                    updated_at: group.updated_at,
                },
                members: members
                    .into_iter()
                    .map(|m| GroupStateMember {
                        member_id: m.member_id,
                        display_name: m.display_name,
                        role: m.role,
                        joined_at: m.joined_at,
                    })
                    .collect(),
                shelf: shelf
                    .into_iter()
                    .map(|s| GroupStateShelf {
                        member_id: s.member_id,
                        work_key: s.work_key,
                        title: s.title,
                        content_type: s.content_type,
                        status: s.status,
                        progress: s.progress,
                        updated_at: s.updated_at,
                    })
                    .collect(),
                notes: notes
                    .into_iter()
                    .map(|n| GroupStateNote {
                        id: n.id,
                        work_key: n.work_key,
                        author_id: n.author_id,
                        body: n.body,
                        created_at: n.created_at,
                        updated_at: n.updated_at,
                    })
                    .collect(),
            });
        }
        Ok(GroupState {
            format: STATE_FORMAT.to_string(),
            version: STATE_VERSION,
            exported_at: now(),
            groups: entries,
        })
    }

    /// Write the state to `path` atomically (`.partial` + rename).
    pub async fn write_state(
        &self,
        path: &Path,
        group_id: Option<&str>,
    ) -> Result<GroupExportReport, AppError> {
        let state = self.export_state(group_id).await?;
        let json = serde_json::to_string_pretty(&state)?;

        let mut partial = path.as_os_str().to_os_string();
        partial.push(".partial");
        let partial = std::path::PathBuf::from(partial);
        std::fs::write(&partial, json.as_bytes()).map_err(AppError::from)?;
        std::fs::rename(&partial, path).map_err(AppError::from)?;

        let groups = state.groups.len();
        let members = state.groups.iter().map(|g| g.members.len()).sum();
        let shelf = state.groups.iter().map(|g| g.shelf.len()).sum();
        let notes = state.groups.iter().map(|g| g.notes.len()).sum();
        Ok(GroupExportReport {
            path: path.display().to_string(),
            groups,
            members,
            shelf,
            notes,
        })
    }

    /// Merge a `group_state.json` payload into the local database.
    ///
    /// Idempotent: groups/shelf/notes are applied last-write-wins by
    /// `updated_at` (local wins on ties), members are upserted. Re-importing the
    /// same file is a no-op.
    pub async fn import_state(&self, source: &str) -> Result<GroupImportReport, AppError> {
        let state: GroupState = serde_json::from_str(source)
            .map_err(|e| AppError::validation(format!("invalid group state: {e}")))?;
        if state.format != STATE_FORMAT {
            return Err(AppError::validation(
                "not a MyLore reading-group state file",
            ));
        }
        if state.version != STATE_VERSION {
            return Err(AppError::validation(format!(
                "unsupported group state version: {}",
                state.version
            )));
        }

        let mut report = GroupImportReport {
            groups: 0,
            members: 0,
            shelf: 0,
            notes: 0,
            skipped: 0,
        };

        for entry in state.groups {
            // Group: insert when missing; otherwise update the name only when the
            // incoming copy is strictly newer (the epoch is local key-rotation state).
            match repo::get_group(&self.pool, &entry.group.id).await? {
                None => {
                    repo::create_group(
                        &self.pool,
                        &GroupRecord {
                            id: entry.group.id.clone(),
                            name: entry.group.name.clone(),
                            owner_id: entry.group.owner_id.clone(),
                            epoch: entry.group.epoch,
                            created_at: entry.group.created_at.clone(),
                            updated_at: entry.group.updated_at.clone(),
                        },
                    )
                    .await?;
                    report.groups += 1;
                }
                Some(existing) => {
                    if entry.group.updated_at > existing.updated_at
                        && !entry.group.name.trim().is_empty()
                    {
                        repo::rename_group(
                            &self.pool,
                            &entry.group.id,
                            entry.group.name.trim(),
                            &entry.group.updated_at,
                        )
                        .await?;
                        report.groups += 1;
                    } else {
                        report.skipped += 1;
                    }
                }
            }

            for member in entry.members {
                let role = MemberRole::from_str(&member.role)
                    .map_err(AppError::from)?
                    .as_str()
                    .to_string();
                repo::add_member(
                    &self.pool,
                    &MemberRecord {
                        group_id: entry.group.id.clone(),
                        member_id: member.member_id,
                        display_name: member.display_name,
                        role,
                        joined_at: member.joined_at,
                    },
                )
                .await?;
                report.members += 1;
            }

            for row in entry.shelf {
                let exists = repo::list_shelf(&self.pool, &entry.group.id, Some(&row.member_id))
                    .await?
                    .into_iter()
                    .find(|s| s.work_key == row.work_key);
                let apply = match &exists {
                    Some(current) => row.updated_at > current.updated_at,
                    None => true,
                };
                if !apply {
                    report.skipped += 1;
                    continue;
                }
                repo::upsert_shelf(
                    &self.pool,
                    &ShelfRecord {
                        group_id: entry.group.id.clone(),
                        member_id: row.member_id,
                        work_key: row.work_key,
                        title: row.title,
                        content_type: row.content_type,
                        status: row.status,
                        progress: row.progress,
                        updated_at: row.updated_at,
                    },
                )
                .await?;
                report.shelf += 1;
            }

            for note in entry.notes {
                match repo::get_note(&self.pool, &note.id).await? {
                    None => {
                        repo::insert_note(
                            &self.pool,
                            &NoteRecord {
                                id: note.id,
                                group_id: entry.group.id.clone(),
                                work_key: note.work_key,
                                author_id: note.author_id,
                                body: note.body,
                                created_at: note.created_at,
                                updated_at: note.updated_at,
                            },
                        )
                        .await?;
                        report.notes += 1;
                    }
                    Some(current) if note.updated_at > current.updated_at => {
                        repo::update_note(&self.pool, &note.id, note.body.trim(), &note.updated_at)
                            .await?;
                        report.notes += 1;
                    }
                    Some(_) => report.skipped += 1,
                }
            }
        }

        Ok(report)
    }
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn shelf_view(s: ShelfRecord) -> GroupShelfEntryView {
    GroupShelfEntryView {
        group_id: s.group_id,
        member_id: s.member_id,
        work_key: s.work_key,
        title: s.title,
        content_type: s.content_type,
        status: s.status,
        progress: s.progress,
        updated_at: s.updated_at,
    }
}

fn note_view(n: NoteRecord) -> GroupNoteView {
    GroupNoteView {
        id: n.id,
        group_id: n.group_id,
        work_key: n.work_key,
        author_id: n.author_id,
        body: n.body,
        created_at: n.created_at,
        updated_at: n.updated_at,
    }
}

fn note_record(n: &GroupNote) -> NoteRecord {
    NoteRecord {
        id: n.id.clone(),
        group_id: n.group_id.clone(),
        work_key: n.work_key.clone(),
        author_id: n.author_id.clone(),
        body: n.body.clone(),
        created_at: n.created_at.clone(),
        updated_at: n.updated_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn service(name: &str) -> (ReadingGroupService, std::path::PathBuf) {
        let (pool, path) = migrated_pool(name).await;
        (ReadingGroupService::new(pool), path)
    }

    #[tokio::test]
    async fn prefs_default_off_and_mint_a_stable_member_id() {
        let (svc, path) = service("rg_prefs.db").await;
        let first = svc.prefs().await.expect("prefs");
        assert!(!first.enabled, "opt-in is off by default");
        assert!(first.member_id.starts_with("m-"));
        // Second read returns the same minted id.
        assert_eq!(svc.prefs().await.unwrap().member_id, first.member_id);

        let updated = svc.set_prefs(true, "Reader").await.expect("set");
        assert!(updated.enabled);
        assert_eq!(updated.display_name, "Reader");
        // Enabling keeps the same identity.
        assert_eq!(updated.member_id, first.member_id);
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn create_group_seeds_the_owner_member() {
        let (svc, path) = service("rg_create.db").await;
        let view = svc.create_group("  Book Club  ").await.expect("create");
        assert_eq!(view.name, "Book Club", "name trimmed");
        assert_eq!(view.members.len(), 1);
        assert_eq!(view.members[0].role, "owner");
        assert_eq!(view.members[0].member_id, view.owner_id);
        assert_eq!(view.epoch, 0);

        assert!(
            svc.create_group("   ").await.is_err(),
            "blank name rejected"
        );
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn shelf_is_single_writer_and_requires_a_member() {
        let (svc, path) = service("rg_shelf.db").await;
        let group = svc.create_group("G").await.expect("create");
        let owner = group.owner_id.clone();

        let entry = svc
            .set_shelf_entry(
                &group.id,
                &owner,
                "anilist:42",
                "Berserk",
                "manga",
                "in_progress",
                3,
            )
            .await
            .expect("shelf");
        assert_eq!(entry.status, "in_progress");
        // Overwrite by the same member (single writer).
        let entry = svc
            .set_shelf_entry(
                &group.id,
                &owner,
                "anilist:42",
                "Berserk",
                "manga",
                "completed",
                41,
            )
            .await
            .expect("re-shelf");
        assert_eq!(entry.progress, 41);
        assert_eq!(svc.shelf(&group.id, Some(&owner)).await.unwrap().len(), 1);

        // Unknown member / status rejected.
        assert!(svc
            .set_shelf_entry(&group.id, "m-ghost", "w", "T", "manga", "planned", 0)
            .await
            .is_err());
        assert!(svc
            .set_shelf_entry(&group.id, &owner, "w", "T", "manga", "nonsense", 0)
            .await
            .is_err());
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn notes_require_a_member_and_roundtrip() {
        let (svc, path) = service("rg_notes.db").await;
        let group = svc.create_group("G").await.expect("create");
        let owner = group.owner_id.clone();

        let note = svc
            .add_note(&group.id, "anilist:42", &owner, "Great chapter")
            .await
            .expect("note");
        assert_eq!(
            svc.notes(&group.id, Some("anilist:42"))
                .await
                .unwrap()
                .len(),
            1
        );

        let updated = svc
            .update_note(&note.id, "Even better")
            .await
            .expect("update");
        assert_eq!(updated.body, "Even better");
        svc.delete_note(&note.id).await.expect("delete");
        assert!(svc.notes(&group.id, None).await.unwrap().is_empty());

        assert!(
            svc.add_note(&group.id, "w", "m-ghost", "hi").await.is_err(),
            "author must be a member"
        );
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn remove_member_rejects_the_owner_and_bumps_the_epoch() {
        let (svc, path) = service("rg_members.db").await;
        let group = svc.create_group("G").await.expect("create");
        let owner = group.owner_id.clone();

        assert!(
            svc.remove_member(&group.id, &owner).await.is_err(),
            "owner cannot be removed"
        );

        svc.add_member(&group.id, "m-2", "Friend", "member")
            .await
            .expect("add");
        let view = svc.remove_member(&group.id, "m-2").await.expect("remove");
        assert_eq!(view.members.len(), 1, "only the owner remains");
        assert_eq!(view.epoch, 1, "epoch bumped for key rotation");
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn export_then_import_roundtrips_and_reimport_is_idempotent() {
        let (svc, path) = service("rg_export.db").await;
        let group = svc.create_group("G").await.expect("create");
        let owner = group.owner_id.clone();
        svc.add_member(&group.id, "m-2", "Friend", "member")
            .await
            .expect("member");
        svc.set_shelf_entry(
            &group.id,
            &owner,
            "anilist:42",
            "Berserk",
            "manga",
            "in_progress",
            3,
        )
        .await
        .expect("shelf");
        svc.add_note(&group.id, "anilist:42", &owner, "hi")
            .await
            .expect("note");

        let state = svc.export_state(Some(&group.id)).await.expect("export");
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(state.groups.len(), 1);
        assert_eq!(state.groups[0].members.len(), 2);
        assert_eq!(state.groups[0].shelf.len(), 1);
        assert_eq!(state.groups[0].notes.len(), 1);

        // Re-import into the same DB: group + shelf + note are all equal-or-older
        // → skipped (members are idempotently upserted).
        let report = svc.import_state(&json).await.expect("re-import");
        assert_eq!(report.skipped, 3, "group, shelf and note skipped");

        // Wipe and import fresh: the world returns.
        repo::delete_group(&svc.pool, &group.id)
            .await
            .expect("wipe");
        assert!(svc.list_groups().await.unwrap().is_empty());
        let report = svc.import_state(&json).await.expect("import");
        assert_eq!(report.groups, 1);
        assert_eq!(report.members, 2);
        assert_eq!(report.shelf, 1);
        assert_eq!(report.notes, 1);

        let restored = svc.view_group(&group.id).await.expect("restored");
        assert_eq!(restored.members.len(), 2);
        assert_eq!(restored.shelf_count, 1);
        assert_eq!(restored.note_count, 1);
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn import_rejects_a_foreign_or_wrong_version_document() {
        let (svc, path) = service("rg_import_bad.db").await;
        assert!(svc.import_state("not json").await.is_err());
        assert!(svc
            .import_state(r#"{"format":"other","version":1,"exported_at":"x","groups":[]}"#)
            .await
            .is_err());
        assert!(svc
            .import_state(
                r#"{"format":"mylore.reading-group-state","version":99,"exported_at":"x","groups":[]}"#
            )
            .await
            .is_err());
        cleanup_files(&path);
    }
}
