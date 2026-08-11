//! Domain enums (MISSION-022).
//!
//! Every enum mirrors a `CHECK` constraint in the SQL schema (DATABASE.md §3),
//! so a value that passes the domain enum is guaranteed to pass the database
//! CHECK. `as_str()` returns the exact storage string; `FromStr`/`TryFrom`
//! parse it back.

use std::str::FromStr;

use crate::domain::error::DomainError;

/// Generate a `CHECK`-aligned enum with `as_str`, `FromStr` and `TryFrom`.
macro_rules! string_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident = $s:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// The exact string stored in SQLite.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $s),+
                }
            }

            /// All valid values, in schema order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
        }

        impl FromStr for $name {
            type Err = DomainError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($s => Ok(Self::$variant),)+
                    _ => Err(DomainError::validation(format!(
                        "invalid {} value: {s:?}",
                        stringify!($name)
                    ))),
                }
            }
        }

        impl TryFrom<&str> for $name {
            type Error = DomainError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                s.parse()
            }
        }

        impl TryFrom<String> for $name {
            type Error = DomainError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                s.parse()
            }
        }
    };
}

string_enum! {
    /// What kind of work a media is (media.content_type CHECK).
    ContentType {
        Book = "book",
        Novel = "novel",
        WebNovel = "web_novel",
        Manga = "manga",
        Manhwa = "manhwa",
        Manhua = "manhua",
        Anime = "anime",
        Tv = "tv",
        Movie = "movie",
        Other = "other",
    }
}

string_enum! {
    /// Publication / airing status (media.pub_status CHECK).
    MediaStatus {
        Announced = "announced",
        Ongoing = "ongoing",
        Completed = "completed",
        Hiatus = "hiatus",
        Cancelled = "cancelled",
        Unknown = "unknown",
    }
}

string_enum! {
    /// Kind of a content node (content_node.kind CHECK).
    NodeKind {
        Season = "season",
        Episode = "episode",
        Volume = "volume",
        Chapter = "chapter",
        PageRange = "page_range",
        Track = "track",
        Issue = "issue",
        Node = "node",
    }
}

string_enum! {
    /// Per-node user state (node_progress.state CHECK).
    NodeProgressState {
        Unread = "unread",
        Read = "read",
        Watched = "watched",
        Skipped = "skipped",
        Partial = "partial",
    }
}

impl NodeProgressState {
    /// A state that marks the node as consumed (DOMAIN_MODEL §2.3).
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Read | Self::Watched)
    }
}

string_enum! {
    /// Core tracking statuses (status.bucket / tracking.core_status CHECK).
    /// The full transition engine builds on these in MISSION-024.
    CoreStatus {
        Planned = "planned",
        InProgress = "in_progress",
        Completed = "completed",
        OnHold = "on_hold",
        Dropped = "dropped",
        Repeat = "repeat",
        Wishlist = "wishlist",
    }
}

impl CoreStatus {
    /// Terminal buckets: the media is finished (completed / dropped).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Dropped)
    }

    /// Active buckets: content is being consumed right now.
    pub fn is_active(self) -> bool {
        matches!(self, Self::InProgress | Self::Repeat)
    }

    /// Not-started buckets: nothing has been consumed yet.
    pub fn is_not_started(self) -> bool {
        matches!(self, Self::Planned | Self::Wishlist)
    }
}

string_enum! {
    /// Person roles on media (person.role CHECK).
    PersonRole {
        Author = "author",
        Artist = "artist",
        Director = "director",
        Studio = "studio",
        Publisher = "publisher",
        Network = "network",
    }
}

string_enum! {
    /// Directed relationship between two media (media_relation.relation CHECK).
    MediaRelationKind {
        Sequel = "sequel",
        Prequel = "prequel",
        Adaptation = "adaptation",
        SameUniverse = "same_universe",
        SpinOff = "spin_off",
        Other = "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_and_parse_roundtrip() {
        for value in ContentType::ALL {
            assert_eq!(ContentType::from_str(value.as_str()).unwrap(), *value);
        }
        assert_eq!(
            MediaStatus::from_str("unknown").unwrap(),
            MediaStatus::Unknown
        );
    }

    #[test]
    fn unknown_values_are_rejected() {
        assert!(ContentType::from_str("podcast").is_err());
        assert!(MediaStatus::from_str("watching").is_err());
        assert!(NodeKind::from_str("arc").is_err());
        assert!(NodeProgressState::from_str("finished").is_err());
        assert!(CoreStatus::from_str("watching").is_err());
        assert!(PersonRole::from_str("editor").is_err());
        assert!(MediaRelationKind::from_str("friends").is_err());
    }

    #[test]
    fn schema_strings_match_check_constraints() {
        assert_eq!(
            ContentType::ALL
                .iter()
                .map(|v| v.as_str())
                .collect::<Vec<_>>(),
            [
                "book",
                "novel",
                "web_novel",
                "manga",
                "manhwa",
                "manhua",
                "anime",
                "tv",
                "movie",
                "other"
            ]
        );
        assert_eq!(
            CoreStatus::ALL
                .iter()
                .map(|v| v.as_str())
                .collect::<Vec<_>>(),
            [
                "planned",
                "in_progress",
                "completed",
                "on_hold",
                "dropped",
                "repeat",
                "wishlist"
            ]
        );
        assert_eq!(
            NodeProgressState::ALL
                .iter()
                .map(|v| v.as_str())
                .collect::<Vec<_>>(),
            ["unread", "read", "watched", "skipped", "partial"]
        );
    }

    #[test]
    fn completed_states_are_consuming() {
        assert!(NodeProgressState::Read.is_completed());
        assert!(NodeProgressState::Watched.is_completed());
        for state in [
            NodeProgressState::Unread,
            NodeProgressState::Skipped,
            NodeProgressState::Partial,
        ] {
            assert!(!state.is_completed());
        }
    }
}
