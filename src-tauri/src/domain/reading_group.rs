//! Reading groups — local domain model (MISSION-114).
//!
//! Pure and side-effect free. A reading group is a **separate aggregate**
//! (ADR-007): it never mixes into the personal user data (`tracking`/`review`).
//! To work across devices — where each install has different `media` UUIDs —
//! group rows reference a work by a stable [`work_key`], not by `media.id`.
//!
//! What ships here is only the local model plus the manual `group_state.json`
//! seam. CRDT notes, E2EE and the Nostr transport arrive in MISSION-115/116/117.

use std::str::FromStr;

use crate::domain::enums::CoreStatus;
use crate::domain::error::DomainError;
use crate::domain::normalize::fold_title;
use crate::domain::value_objects::ExternalId;

/// Who owns a group row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRole {
    Owner,
    Member,
}

impl MemberRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Member => "member",
        }
    }
}

impl FromStr for MemberRole {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(Self::Owner),
            "member" => Ok(Self::Member),
            other => Err(DomainError::validation(format!(
                "invalid member role: {other:?}"
            ))),
        }
    }
}

/// A reading group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub epoch: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl Group {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.id.trim().is_empty() {
            return Err(DomainError::validation("group id must not be empty"));
        }
        if self.name.trim().is_empty() {
            return Err(DomainError::validation("group name must not be empty"));
        }
        if self.owner_id.trim().is_empty() {
            return Err(DomainError::validation("group owner must not be empty"));
        }
        if self.epoch < 0 {
            return Err(DomainError::validation("group epoch must not be negative"));
        }
        Ok(())
    }
}

/// A group membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMember {
    pub group_id: String,
    pub member_id: String,
    pub display_name: String,
    pub role: MemberRole,
    pub joined_at: String,
}

impl GroupMember {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.group_id.trim().is_empty() || self.member_id.trim().is_empty() {
            return Err(DomainError::validation(
                "group_id and member_id must not be empty",
            ));
        }
        if self.display_name.trim().is_empty() {
            return Err(DomainError::validation(
                "member display name must not be empty",
            ));
        }
        Ok(())
    }
}

/// One member's shelf entry for a work (single-writer per member).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfEntry {
    pub group_id: String,
    pub member_id: String,
    pub work_key: String,
    pub title: String,
    pub content_type: String,
    pub status: CoreStatus,
    pub progress: i64,
    pub updated_at: String,
}

impl ShelfEntry {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.group_id.trim().is_empty()
            || self.member_id.trim().is_empty()
            || self.work_key.trim().is_empty()
        {
            return Err(DomainError::validation(
                "group_id, member_id and work_key must not be empty",
            ));
        }
        if self.title.trim().is_empty() {
            return Err(DomainError::validation(
                "shelf entry title must not be empty",
            ));
        }
        if self.progress < 0 {
            return Err(DomainError::validation(
                "shelf progress must not be negative",
            ));
        }
        Ok(())
    }
}

/// A shared note on a work inside a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupNote {
    pub id: String,
    pub group_id: String,
    pub work_key: String,
    pub author_id: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

impl GroupNote {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.id.trim().is_empty()
            || self.group_id.trim().is_empty()
            || self.work_key.trim().is_empty()
            || self.author_id.trim().is_empty()
        {
            return Err(DomainError::validation(
                "note id, group_id, work_key and author_id must not be empty",
            ));
        }
        if self.body.trim().is_empty() {
            return Err(DomainError::validation("note body must not be empty"));
        }
        Ok(())
    }
}

/// Stable, device-independent identity for a work.
///
/// Prefers a `{provider}:{value}` external id (the smallest pair, so the result
/// is deterministic regardless of the order ids arrive in); otherwise falls
/// back to a hash of the folded `title|author|year`. Two installs therefore
/// agree on the same key for the same work even though their `media` UUIDs
/// differ.
pub fn work_key(
    external_ids: &[ExternalId],
    title: &str,
    author: Option<&str>,
    year: Option<i64>,
) -> String {
    if let Some(id) = external_ids
        .iter()
        .map(|e| (e.provider().as_str(), e.value()))
        .min()
    {
        return format!("{}:{}", id.0, id.1);
    }
    let author = author.map(fold_title).unwrap_or_default();
    let basis = format!(
        "{}|{}|{}",
        fold_title(title),
        author,
        year.map(|y| y.to_string()).unwrap_or_default()
    );
    format!("h:{:016x}", fnv1a64(&basis))
}

/// FNV-1a 64-bit — a small, deterministic hash (unlike `std::hash`, it is not
/// seeded per process and never changes between Rust versions).
fn fnv1a64(input: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::ProviderId;

    fn ext(provider: &str, value: &str) -> ExternalId {
        ExternalId::new(ProviderId::new(provider).unwrap(), value, None).unwrap()
    }

    #[test]
    fn work_key_prefers_a_provider_id_deterministically() {
        let ids = [ext("mangadex", "abc"), ext("anilist", "42")];
        // Smallest (provider, value) pair wins regardless of input order.
        assert_eq!(work_key(&ids, "Title", None, None), "anilist:42");
        let reversed = [ext("anilist", "42"), ext("mangadex", "abc")];
        assert_eq!(work_key(&reversed, "Title", None, None), "anilist:42");
    }

    #[test]
    fn work_key_hash_fallback_is_stable_and_folded() {
        let a = work_key(&[], "Sword of the Dawn", Some("A. Author"), Some(2025));
        let b = work_key(&[], "Sword of the  Dawn", Some("a. author"), Some(2025));
        // fold_title collapses case/whitespace → same key.
        assert_eq!(a, b);
        assert!(a.starts_with("h:"));
        // A different year (or title) changes the key.
        assert_ne!(
            a,
            work_key(&[], "Sword of the Dawn", Some("A. Author"), Some(2024))
        );
        assert_ne!(
            a,
            work_key(&[], "Other Title", Some("A. Author"), Some(2025))
        );
    }

    #[test]
    fn work_key_hash_is_device_independent_shape() {
        let key = work_key(&[], "Berserk", None, Some(1989));
        assert_eq!(key.len(), 2 + 16, "h: + 16 hex chars");
    }

    #[test]
    fn group_validate_rejects_blank_name_and_negative_epoch() {
        let base = Group {
            id: "g-1".into(),
            name: "Book Club".into(),
            owner_id: "m-1".into(),
            epoch: 0,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        };
        assert!(base.validate().is_ok());
        assert!(Group {
            name: "  ".into(),
            ..base.clone()
        }
        .validate()
        .is_err());
        assert!(Group { epoch: -1, ..base }.validate().is_err());
    }

    #[test]
    fn shelf_entry_validate_checks_status_and_progress() {
        let entry = ShelfEntry {
            group_id: "g-1".into(),
            member_id: "m-1".into(),
            work_key: "anilist:42".into(),
            title: "Berserk".into(),
            content_type: "manga".into(),
            status: CoreStatus::InProgress,
            progress: 3,
            updated_at: "2026-01-01".into(),
        };
        assert!(entry.validate().is_ok());
        assert!(ShelfEntry {
            progress: -1,
            ..entry.clone()
        }
        .validate()
        .is_err());
        assert!(ShelfEntry {
            title: " ".into(),
            ..entry
        }
        .validate()
        .is_err());
    }

    #[test]
    fn note_validate_rejects_empty_body() {
        let note = GroupNote {
            id: "n-1".into(),
            group_id: "g-1".into(),
            work_key: "anilist:42".into(),
            author_id: "m-1".into(),
            body: "Great chapter!".into(),
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        };
        assert!(note.validate().is_ok());
        assert!(GroupNote {
            body: "   ".into(),
            ..note
        }
        .validate()
        .is_err());
    }

    #[test]
    fn member_role_roundtrips() {
        assert_eq!(MemberRole::from_str("owner").unwrap(), MemberRole::Owner);
        assert_eq!(MemberRole::from_str("member").unwrap(), MemberRole::Member);
        assert!(MemberRole::from_str("admin").is_err());
    }
}
