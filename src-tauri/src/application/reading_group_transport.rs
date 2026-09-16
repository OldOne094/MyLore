//! Reading-groups relay transport (MISSION-116).
//!
//! Behind the `p2p` feature, same as the engine it drives. The shape here is
//! deliberately narrow so the network is swappable: a [`GroupTransport`] moves
//! *already sealed* envelopes, and everything else — the outbox, dedup, merge —
//! is local database work that a test can drive with an in-memory transport.
//!
//! - **Outbox-first:** a local edit is written to `group_outbox` before any
//!   relay is contacted. A flush marks rows sent; a failure leaves them pending
//!   for the next attempt, so an offline window never loses a change.
//! - **Idempotent:** a re-delivered envelope is claimed once (by message id),
//!   so pulling the same events twice does not disturb the document.
//! - **Chunked:** an envelope larger than one relay event is split into
//!   `MLC1`-headed chunks and reassembled on arrival.

use crate::error::AppError;

/// A group's relays plus how many envelopes are still waiting to go out.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupRelayView {
    pub relays: Vec<String>,
    pub pending: i64,
}

/// What one sync pass did.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupSyncReport {
    /// Envelopes handed to the relay.
    pub published: i64,
    /// Envelopes the relay refused (they stay pending).
    pub failed: i64,
    /// Whole envelopes reassembled from the relay.
    pub received: i64,
    /// Envelopes merged into the local document.
    pub merged: i64,
    /// Envelopes dropped as already seen.
    pub skipped: i64,
    /// Envelopes still waiting after this pass.
    pub pending: i64,
}

#[cfg(feature = "p2p")]
mod imp {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;

    use nostr_sdk::prelude::{
        Client, Event, EventBuilder, Filter, FinalizeEvent, Keys, Kind, SingleLetterTag, Tag,
    };
    use sqlx::SqlitePool;

    use super::*;
    use crate::application::reading_group_p2p;
    use crate::application::reading_group_service::ReadingGroupService;
    use crate::application::task_service::TaskManager;
    use crate::domain::task::{TaskError, TaskKind, TaskSnapshot};
    use crate::error::AppError;
    use crate::infrastructure::keyring::SecretStore;
    use crate::infrastructure::repositories::reading_group as repo;
    use crate::infrastructure::repositories::reading_group::OutboxRecord;

    /// The Nostr event kind MyLore uses for group traffic. Application-specific
    /// (regular) range; the payload is sealed regardless, so relays can only
    /// see metadata.
    pub const GROUP_EVENT_KIND: u16 = 21337;
    /// Event content budget: relays commonly reject >64KB events, and the
    /// 28-byte chunk header plus base64 inflation have to fit inside it.
    pub const MAX_EVENT_BYTES: usize = 60 * 1024;
    const CHUNK_MAGIC: &[u8; 4] = b"MLC1";
    /// magic(4) + message id(16) + seq(4) + total(4).
    const CHUNK_HEADER: usize = 28;
    const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
    const FETCH_LIMIT: usize = 500;
    const NOSTR_KEY_ENTRY: &str = "readingGroup.nostrKey";
    /// Keep the relay set small: every envelope goes to every relay.
    const MAX_RELAYS: usize = 8;

    /// One whole envelope that arrived from a relay.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Incoming {
        pub work_key: String,
        pub message_id: String,
        pub envelope: Vec<u8>,
    }

    /// Moves sealed envelopes to and from a group's relays.
    #[async_trait::async_trait]
    pub trait GroupTransport: Send + Sync {
        /// Broadcast one envelope. Implementations chunk as needed.
        async fn publish(
            &self,
            relays: &[String],
            group_id: &str,
            work_key: &str,
            message_id: &str,
            envelope: &[u8],
        ) -> Result<(), AppError>;

        /// Every whole envelope currently readable for `group_id`.
        async fn fetch(&self, relays: &[String], group_id: &str)
            -> Result<Vec<Incoming>, AppError>;
    }

    // --------------------------------------------------------------- chunking

    fn b64(bytes: &[u8]) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn message_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Split a sealed envelope into relay-sized chunks, each self-describing.
    pub fn chunk_payload(payload: &[u8], max_body: usize) -> Vec<Vec<u8>> {
        assert!(max_body > 0, "chunk body budget must be positive");
        let total = payload.len().div_ceil(max_body).max(1);
        let mut chunks = Vec::with_capacity(total);
        let id = uuid::Uuid::new_v4();
        for seq in 0..total {
            let start = seq * max_body;
            let end = (start + max_body).min(payload.len());
            let body = payload.get(start..end).unwrap_or(&[]);
            let mut chunk = Vec::with_capacity(CHUNK_HEADER + body.len());
            chunk.extend_from_slice(CHUNK_MAGIC);
            chunk.extend_from_slice(id.as_bytes());
            chunk.extend_from_slice(&(seq as u32).to_le_bytes());
            chunk.extend_from_slice(&(total as u32).to_le_bytes());
            chunk.extend_from_slice(body);
            chunks.push(chunk);
        }
        chunks
    }

    /// The `(message_id, seq, total, body)` a chunk carries.
    fn decode_chunk(chunk: &[u8]) -> Option<(String, u32, u32, &[u8])> {
        if chunk.len() < CHUNK_HEADER || &chunk[..4] != CHUNK_MAGIC {
            return None;
        }
        let id = uuid::Uuid::from_slice(&chunk[4..20]).ok()?;
        let seq = u32::from_le_bytes(chunk[20..24].try_into().ok()?);
        let total = u32::from_le_bytes(chunk[24..28].try_into().ok()?);
        Some((id.to_string(), seq, total, &chunk[CHUNK_HEADER..]))
    }

    /// Reassemble whole envelopes from a bag of chunks. Incomplete groups (a
    /// chunk still in flight, or one the relay dropped) are left out so the next
    /// pass can pick them up.
    pub fn assemble(chunks: &[(String, Vec<u8>)]) -> Vec<Incoming> {
        struct Partial {
            total: u32,
            parts: Vec<(u32, Vec<u8>)>,
        }
        let mut groups: HashMap<(String, String), Partial> = HashMap::new();
        for (work_key, chunk) in chunks {
            let Some((id, seq, total, body)) = decode_chunk(chunk) else {
                continue;
            };
            let entry = groups
                .entry((work_key.clone(), id))
                .or_insert_with(|| Partial {
                    total,
                    parts: Vec::new(),
                });
            entry.parts.push((seq, body.to_vec()));
        }

        let mut out = Vec::new();
        for ((work_key, message_id), mut partial) in groups {
            if partial.parts.len() as u32 != partial.total {
                continue;
            }
            partial.parts.sort_by_key(|(seq, _)| *seq);
            let envelope: Vec<u8> = partial
                .parts
                .iter()
                .flat_map(|(_, body)| body.iter().copied())
                .collect();
            out.push(Incoming {
                work_key,
                message_id,
                envelope,
            });
        }
        out
    }

    /// Normalize a relay list: trimmed, deduped, capped, in a stable order.
    pub fn normalize_relays(relays: Vec<String>) -> Vec<String> {
        let mut relays: Vec<String> = relays
            .into_iter()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect();
        relays.sort();
        relays.dedup();
        relays.truncate(MAX_RELAYS);
        relays
    }

    // ---------------------------------------------------------- nostr backend

    /// Talks to public (or self-hosted) Nostr relays through `nostr-sdk`.
    pub struct NostrTransport {
        client: Client,
        keys: Keys,
    }

    impl NostrTransport {
        /// Build the client, loading (or minting) this install's Nostr identity.
        pub fn new(store: &dyn SecretStore) -> Result<Self, AppError> {
            let keys = nostr_keys(store)?;
            Ok(Self {
                client: Client::default(),
                keys,
            })
        }

        async fn connect(&self, relays: &[String]) {
            for url in relays {
                let _ = self.client.add_relay(url.as_str()).await;
            }
        }

        fn build_event(
            &self,
            group_id: &str,
            work_key: &str,
            chunk: &[u8],
        ) -> Result<Event, AppError> {
            let tags = vec![
                Tag::parse(["g", group_id]).map_err(nostr_err)?,
                Tag::parse(["t", work_key]).map_err(nostr_err)?,
            ];
            EventBuilder::new(Kind::Custom(GROUP_EVENT_KIND), b64(chunk))
                .tags(tags)
                .finalize(&self.keys)
                .map_err(nostr_err)
        }
    }

    #[async_trait::async_trait]
    impl GroupTransport for NostrTransport {
        async fn publish(
            &self,
            relays: &[String],
            group_id: &str,
            work_key: &str,
            _message_id: &str,
            envelope: &[u8],
        ) -> Result<(), AppError> {
            if relays.is_empty() {
                return Err(AppError::validation("no relays configured for this group"));
            }
            self.connect(relays).await;
            for chunk in chunk_payload(envelope, MAX_EVENT_BYTES - CHUNK_HEADER) {
                let event = self.build_event(group_id, work_key, &chunk)?;
                self.client
                    .send_event(&event)
                    .to(relays.to_vec())
                    .await
                    .map_err(nostr_err)?;
            }
            Ok(())
        }

        async fn fetch(
            &self,
            relays: &[String],
            group_id: &str,
        ) -> Result<Vec<Incoming>, AppError> {
            if relays.is_empty() {
                return Ok(Vec::new());
            }
            self.connect(relays).await;
            let filter = Filter::new()
                .kind(Kind::Custom(GROUP_EVENT_KIND))
                .custom_tag(SingleLetterTag::LOWERCASE_G, group_id.to_string())
                .limit(FETCH_LIMIT);
            let events = self
                .client
                .fetch_events(filter)
                .timeout(FETCH_TIMEOUT)
                .await
                .map_err(nostr_err)?;

            let mut chunks: Vec<(String, Vec<u8>)> = Vec::new();
            for event in events {
                let work_key = event
                    .tags
                    .iter()
                    .find(|tag| tag.single_letter_tag() == Some(SingleLetterTag::LOWERCASE_T))
                    .and_then(|tag| tag.content().map(str::to_string));
                let Some(work_key) = work_key else { continue };
                let Ok(chunk) = unb64(&event.content) else {
                    continue;
                };
                chunks.push((work_key, chunk));
            }
            Ok(assemble(&chunks))
        }
    }

    /// This install's Nostr signing key, minted on first use and kept in the
    /// same secret store as the group keys.
    fn nostr_keys(store: &dyn SecretStore) -> Result<Keys, AppError> {
        if let Some(secret) = store.get(NOSTR_KEY_ENTRY).map_err(AppError::internal)? {
            return Keys::parse(secret.trim())
                .map_err(|e| AppError::internal(format!("stored nostr key is invalid: {e}")));
        }
        let keys = Keys::generate();
        store
            .set(NOSTR_KEY_ENTRY, &keys.secret_key().to_secret_hex())
            .map_err(AppError::internal)?;
        Ok(keys)
    }

    fn unb64(text: &str) -> Result<Vec<u8>, AppError> {
        use base64::Engine as _;
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(text.as_bytes())
            .map_err(|e| AppError::validation(format!("invalid relay payload: {e}")))
    }

    fn nostr_err<E: std::fmt::Display>(error: E) -> AppError {
        AppError::validation(format!("relay error: {error}"))
    }

    // ------------------------------------------------------------ in-memory

    #[derive(Debug, Clone)]
    struct StoredChunk {
        group_id: String,
        work_key: String,
        chunk: Vec<u8>,
    }

    /// A relay in a box: chunking and reassembly exactly like the real one, but
    /// nothing leaves the process. Tests use two pools against one instance.
    #[derive(Default)]
    pub struct InMemoryTransport {
        stored: Mutex<Vec<StoredChunk>>,
        online: AtomicBool,
    }

    impl InMemoryTransport {
        pub fn new() -> Self {
            Self {
                stored: Mutex::new(Vec::new()),
                online: AtomicBool::new(true),
            }
        }

        /// Simulate the network being down (a flush must keep its rows pending).
        pub fn set_online(&self, online: bool) {
            self.online.store(online, Ordering::SeqCst);
        }

        /// How many events the relay is holding.
        pub fn event_count(&self) -> usize {
            self.stored.lock().map(|s| s.len()).unwrap_or(0)
        }
    }

    #[async_trait::async_trait]
    impl GroupTransport for InMemoryTransport {
        async fn publish(
            &self,
            _relays: &[String],
            group_id: &str,
            work_key: &str,
            _message_id: &str,
            envelope: &[u8],
        ) -> Result<(), AppError> {
            if !self.online.load(Ordering::SeqCst) {
                return Err(AppError::validation("relay unreachable"));
            }
            let mut stored = self
                .stored
                .lock()
                .map_err(|_| AppError::internal("mock relay poisoned"))?;
            for chunk in chunk_payload(envelope, MAX_EVENT_BYTES - CHUNK_HEADER) {
                stored.push(StoredChunk {
                    group_id: group_id.to_string(),
                    work_key: work_key.to_string(),
                    chunk,
                });
            }
            Ok(())
        }

        async fn fetch(
            &self,
            _relays: &[String],
            group_id: &str,
        ) -> Result<Vec<Incoming>, AppError> {
            if !self.online.load(Ordering::SeqCst) {
                return Err(AppError::validation("relay unreachable"));
            }
            let stored = self
                .stored
                .lock()
                .map_err(|_| AppError::internal("mock relay poisoned"))?;
            let chunks: Vec<(String, Vec<u8>)> = stored
                .iter()
                .filter(|s| s.group_id == group_id)
                .map(|s| (s.work_key.clone(), s.chunk.clone()))
                .collect();
            Ok(assemble(&chunks))
        }
    }

    // ------------------------------------------------------------- use-cases

    /// A group's relays and outbox depth.
    pub async fn relays_view(
        pool: &SqlitePool,
        group_id: &str,
    ) -> Result<GroupRelayView, AppError> {
        Ok(GroupRelayView {
            relays: repo::relays(pool, group_id).await?,
            pending: repo::pending_count(pool, group_id).await?,
        })
    }

    /// Replace a group's relay set. Owner-only: relays are group settings, and
    /// every member's traffic flows through them.
    pub async fn set_relays(
        pool: &SqlitePool,
        group_id: &str,
        relays: Vec<String>,
    ) -> Result<GroupRelayView, AppError> {
        require_owner(pool, group_id).await?;
        let relays = normalize_relays(relays);
        repo::set_relays(pool, group_id, &relays).await?;
        relays_view(pool, group_id).await
    }

    /// Queue a sealed envelope for broadcast (outbox-first).
    pub async fn enqueue_envelope(
        pool: &SqlitePool,
        group_id: &str,
        work_key: &str,
        envelope: &[u8],
    ) -> Result<(), AppError> {
        repo::enqueue(
            pool,
            &OutboxRecord {
                id: format!("o-{}", uuid::Uuid::new_v4()),
                group_id: group_id.to_string(),
                work_key: work_key.to_string(),
                topic: work_key.to_string(),
                message_id: message_id(),
                payload: envelope.to_vec(),
                created_at: chrono::Utc::now().to_rfc3339(),
                sent_at: None,
                attempts: 0,
                last_error: None,
            },
        )
        .await
    }

    /// One sync pass: flush the outbox, then pull and merge what peers sent.
    pub async fn sync_now(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        transport: &dyn GroupTransport,
        group_id: &str,
    ) -> Result<GroupSyncReport, AppError> {
        require_group(pool, group_id).await?;
        let relays = repo::relays(pool, group_id).await?;

        // 1. Flush: whatever is queued goes out before we read anything back.
        let (mut published, mut failed) = (0, 0);
        for row in repo::pending(pool, group_id).await? {
            match transport
                .publish(
                    &relays,
                    group_id,
                    &row.work_key,
                    &row.message_id,
                    &row.payload,
                )
                .await
            {
                Ok(()) => {
                    repo::mark_sent(pool, &row.id, &chrono::Utc::now().to_rfc3339()).await?;
                    published += 1;
                }
                Err(error) => {
                    // Keep the row for the next pass; a partial failure must not
                    // lose the change.
                    repo::mark_failed(pool, &row.id, &error.to_string()).await?;
                    failed += 1;
                }
            }
        }

        // 2. Pull: merge anything we have not already seen.
        let (mut received, mut merged, mut skipped) = (0, 0, 0);
        if failed == 0 {
            for incoming in transport.fetch(&relays, group_id).await? {
                received += 1;
                let claimed = repo::claim_event(
                    pool,
                    &incoming.message_id,
                    group_id,
                    &incoming.work_key,
                    &chrono::Utc::now().to_rfc3339(),
                )
                .await?;
                if !claimed {
                    skipped += 1;
                    continue;
                }
                reading_group_p2p::apply_envelope(
                    pool,
                    store,
                    group_id,
                    &incoming.work_key,
                    &incoming.envelope,
                )
                .await?;
                merged += 1;
            }
        }

        Ok(GroupSyncReport {
            published,
            failed,
            received,
            merged,
            skipped,
            pending: repo::pending_count(pool, group_id).await?,
        })
    }

    async fn require_group(pool: &SqlitePool, group_id: &str) -> Result<(), AppError> {
        match repo::get_group(pool, group_id).await? {
            Some(_) => Ok(()),
            None => Err(AppError::validation(format!("unknown group: {group_id}"))),
        }
    }

    /// Group settings are owner-only (ARCHITECTURE §6).
    async fn require_owner(pool: &SqlitePool, group_id: &str) -> Result<(), AppError> {
        let service = ReadingGroupService::new(pool.clone());
        let group = service.view_group(group_id).await?;
        let prefs = service.prefs().await?;
        if group.owner_id != prefs.member_id {
            return Err(AppError::validation(
                "only the group owner can change the relay settings",
            ));
        }
        Ok(())
    }

    /// Run one sync pass on the task manager (`task-changed` streams progress)
    /// and return the task's initial snapshot.
    pub fn spawn_sync(
        pool: &SqlitePool,
        store: &std::sync::Arc<dyn SecretStore>,
        tasks: &std::sync::Arc<TaskManager>,
        group_id: &str,
    ) -> Result<TaskSnapshot, AppError> {
        let pool = pool.clone();
        let store = store.clone();
        let group_id = group_id.to_string();
        let title = format!("Sync group {group_id}");

        let id = tasks.spawn(TaskKind::GroupSync, title, move |reporter| async move {
            reporter.progress(5, Some("Connecting to relays…".to_string()));
            let transport = NostrTransport::new(store.as_ref())
                .map_err(|e| TaskError::failed(e.to_string()))?;
            let report = sync_now(&pool, store.as_ref(), &transport, &group_id)
                .await
                .map_err(|e| TaskError::failed(e.to_string()))?;
            reporter.progress(100, Some("Sync complete".to_string()));
            serde_json::to_value(report).map_err(|e| TaskError::failed(e.to_string()))
        });

        tasks
            .get(&id)
            .ok_or_else(|| AppError::internal("sync task vanished before it was reported"))
    }
}

#[cfg(not(feature = "p2p"))]
mod imp {
    use sqlx::SqlitePool;

    use super::*;
    use crate::application::task_service::TaskManager;
    use crate::domain::task::TaskSnapshot;
    use crate::error::AppError;
    use crate::infrastructure::keyring::SecretStore;

    fn unsupported<T>() -> Result<T, AppError> {
        Err(AppError::validation(
            "this build was compiled without reading-groups p2p support (feature `p2p`)",
        ))
    }

    pub async fn relays_view(
        _pool: &SqlitePool,
        _group_id: &str,
    ) -> Result<GroupRelayView, AppError> {
        unsupported()
    }

    pub async fn set_relays(
        _pool: &SqlitePool,
        _group_id: &str,
        _relays: Vec<String>,
    ) -> Result<GroupRelayView, AppError> {
        unsupported()
    }

    pub async fn enqueue_envelope(
        _pool: &SqlitePool,
        _group_id: &str,
        _work_key: &str,
        _envelope: &[u8],
    ) -> Result<(), AppError> {
        // A default build never reaches here (the edit that would queue it
        // already failed), but the call site must still compile.
        Ok(())
    }

    pub fn spawn_sync(
        _pool: &SqlitePool,
        _store: &std::sync::Arc<dyn SecretStore>,
        _tasks: &std::sync::Arc<TaskManager>,
        _group_id: &str,
    ) -> Result<TaskSnapshot, AppError> {
        unsupported()
    }
}

#[cfg(not(feature = "p2p"))]
pub use imp::spawn_sync;
pub use imp::{enqueue_envelope, relays_view, set_relays};
#[cfg(feature = "p2p")]
pub use imp::{
    spawn_sync, sync_now, GroupTransport, InMemoryTransport, NostrTransport, GROUP_EVENT_KIND,
};

/// Decode the base64 update an engine view carries back into raw bytes.
pub fn envelope_bytes(update_b64: &str) -> Result<Vec<u8>, AppError> {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(update_b64.as_bytes())
        .map_err(|e| AppError::validation(format!("invalid sealed envelope: {e}")))
}

#[cfg(all(test, feature = "p2p"))]
mod tests {
    use super::*;
    use crate::application::reading_group_p2p::{
        invite_accept, invite_create, note_edit, note_state, GroupNoteEntry,
    };
    use crate::application::reading_group_service::ReadingGroupService;
    use crate::infrastructure::keyring::InMemoryKeyring;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    struct Device {
        pool: sqlx::SqlitePool,
        store: InMemoryKeyring,
        path: std::path::PathBuf,
    }

    async fn device(name: &str) -> Device {
        let (pool, path) = migrated_pool(name).await;
        Device {
            pool,
            store: InMemoryKeyring::new(),
            path,
        }
    }

    /// Queue one local edit the way the command layer does.
    async fn edit_and_queue(d: &Device, group_id: &str, note_id: &str, body: &str) {
        let view = note_edit(&d.pool, &d.store, group_id, "w", note_id, body)
            .await
            .expect("edit");
        let envelope = envelope_bytes(&view.update).unwrap();
        enqueue_envelope(&d.pool, group_id, "w", &envelope)
            .await
            .expect("enqueue");
    }

    /// Note ids the device currently holds.
    async fn note_ids(d: &Device, group_id: &str) -> Vec<String> {
        note_state(&d.pool, group_id, "w")
            .await
            .unwrap()
            .into_iter()
            .map(|n| n.note_id)
            .collect()
    }

    fn entry(note_id: &str, body: &str) -> GroupNoteEntry {
        GroupNoteEntry {
            note_id: note_id.to_string(),
            body: body.to_string(),
        }
    }

    // ------------------------------------------------------------- chunking

    #[test]
    fn chunks_round_trip_and_hold_back_incomplete_groups() {
        let payload: Vec<u8> = (0..10_000u32).flat_map(|i| i.to_le_bytes()).collect();
        let chunks = imp::chunk_payload(&payload, 16 * 1024);
        assert_eq!(
            chunks.len(),
            3,
            "40KB payload splits into three 16KB chunks"
        );
        assert!(
            chunks.iter().all(|c| c.len() <= 16 * 1024 + 28),
            "no chunk exceeds the body budget plus its header"
        );

        let bag: Vec<(String, Vec<u8>)> = chunks
            .iter()
            .map(|c| ("w".to_string(), c.clone()))
            .collect();
        let assembled = imp::assemble(&bag);
        assert_eq!(assembled.len(), 1);
        assert_eq!(assembled[0].work_key, "w");
        assert_eq!(assembled[0].envelope, payload);

        // An empty envelope still travels as one (empty) chunk.
        let empty = imp::chunk_payload(&[], 16 * 1024);
        assert_eq!(empty.len(), 1);
        let assembled = imp::assemble(&[("w".to_string(), empty[0].clone())]);
        assert_eq!(assembled.len(), 1);
        assert!(assembled[0].envelope.is_empty());

        // A missing chunk withholds the whole envelope — the next pass retries.
        let partial: Vec<(String, Vec<u8>)> = chunks[..2]
            .iter()
            .map(|c| ("w".to_string(), c.clone()))
            .collect();
        assert!(imp::assemble(&partial).is_empty());

        // Foreign junk is ignored rather than panicking.
        assert!(imp::assemble(&[("w".to_string(), b"not a chunk".to_vec())]).is_empty());
    }

    #[test]
    fn relays_are_normalized() {
        assert_eq!(
            imp::normalize_relays(vec![
                "  wss://b.example ".into(),
                "wss://a.example".into(),
                "wss://b.example".into(),
                "   ".into(),
            ]),
            vec!["wss://a.example".to_string(), "wss://b.example".to_string()]
        );
    }

    // -------------------------------------------------------------- relay

    #[tokio::test]
    async fn a_peer_receives_a_note_written_while_it_was_away() {
        let a = device("rg_tx_a.db").await;
        let b = device("rg_tx_b.db").await;
        let relay = InMemoryTransport::new();

        let group = ReadingGroupService::new(a.pool.clone())
            .create_group("Book Club")
            .await
            .expect("group");
        let invite = invite_create(&a.pool, &a.store, &group.id, vec![])
            .await
            .unwrap();
        invite_accept(&b.pool, &b.store, &invite.link)
            .await
            .unwrap();
        set_relays(&a.pool, &group.id, vec!["wss://relay.test".into()])
            .await
            .unwrap();
        set_relays(&b.pool, &group.id, vec!["wss://relay.test".into()])
            .await
            .unwrap();

        // A writes a note: queued locally first, nothing published yet.
        edit_and_queue(&a, &group.id, "n1", "hello").await;
        assert_eq!(relay.event_count(), 0);
        assert_eq!(relays_view(&a.pool, &group.id).await.unwrap().pending, 1);

        // A syncs (its "session" ends) — only now does the envelope leave.
        let report = sync_now(&a.pool, &a.store, &relay, &group.id)
            .await
            .unwrap();
        assert_eq!((report.published, report.failed, report.pending), (1, 0, 0));

        // B, which was away, syncs later and receives it.
        let report = sync_now(&b.pool, &b.store, &relay, &group.id)
            .await
            .unwrap();
        assert_eq!((report.received, report.merged, report.skipped), (1, 1, 0));
        assert_eq!(
            note_state(&b.pool, &group.id, "w").await.unwrap(),
            vec![entry("n1", "hello")]
        );

        // B replies; A picks it up on its next pass.
        edit_and_queue(&b, &group.id, "n2", "world").await;
        sync_now(&b.pool, &b.store, &relay, &group.id)
            .await
            .unwrap();
        sync_now(&a.pool, &a.store, &relay, &group.id)
            .await
            .unwrap();
        assert_eq!(
            note_state(&a.pool, &group.id, "w").await.unwrap(),
            vec![entry("n1", "hello"), entry("n2", "world")]
        );
        assert_eq!(note_ids(&a, &group.id).await, note_ids(&b, &group.id).await);

        cleanup_files(&a.path);
        cleanup_files(&b.path);
    }

    #[tokio::test]
    async fn an_offline_flush_keeps_the_outbox_and_retries() {
        let a = device("rg_tx_offline.db").await;
        let relay = InMemoryTransport::new();
        let group = ReadingGroupService::new(a.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        set_relays(&a.pool, &group.id, vec!["wss://relay.test".into()])
            .await
            .unwrap();
        edit_and_queue(&a, &group.id, "n1", "queued while offline").await;

        // The relay is unreachable: the row stays pending for the next pass.
        relay.set_online(false);
        let report = sync_now(&a.pool, &a.store, &relay, &group.id)
            .await
            .unwrap();
        assert_eq!((report.published, report.failed, report.pending), (0, 1, 1));

        // Reconnect: the same row goes out, nothing was lost.
        relay.set_online(true);
        let report = sync_now(&a.pool, &a.store, &relay, &group.id)
            .await
            .unwrap();
        assert_eq!((report.published, report.failed, report.pending), (1, 0, 0));
        assert_eq!(relay.event_count(), 1);

        cleanup_files(&a.path);
    }

    #[tokio::test]
    async fn a_re_delivered_envelope_is_merged_once() {
        let a = device("rg_tx_dup_a.db").await;
        let b = device("rg_tx_dup_b.db").await;
        let relay = InMemoryTransport::new();
        let group = ReadingGroupService::new(a.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        let invite = invite_create(&a.pool, &a.store, &group.id, vec![])
            .await
            .unwrap();
        invite_accept(&b.pool, &b.store, &invite.link)
            .await
            .unwrap();
        set_relays(&a.pool, &group.id, vec!["wss://relay.test".into()])
            .await
            .unwrap();
        set_relays(&b.pool, &group.id, vec!["wss://relay.test".into()])
            .await
            .unwrap();

        edit_and_queue(&a, &group.id, "n1", "hello").await;
        sync_now(&a.pool, &a.store, &relay, &group.id)
            .await
            .unwrap();

        let first = sync_now(&b.pool, &b.store, &relay, &group.id)
            .await
            .unwrap();
        assert_eq!((first.merged, first.skipped), (1, 0));
        let before = note_state(&b.pool, &group.id, "w").await.unwrap();

        // The envelope is still on the relay; a second pull must not re-merge it.
        let second = sync_now(&b.pool, &b.store, &relay, &group.id)
            .await
            .unwrap();
        assert_eq!((second.merged, second.skipped), (0, 1));
        assert_eq!(note_state(&b.pool, &group.id, "w").await.unwrap(), before);

        cleanup_files(&a.path);
        cleanup_files(&b.path);
    }

    #[tokio::test]
    async fn only_the_owner_can_change_relays() {
        let a = device("rg_tx_owner.db").await;
        let group = ReadingGroupService::new(a.pool.clone())
            .create_group("G")
            .await
            .expect("group");

        // The owner can set relays…
        assert!(set_relays(&a.pool, &group.id, vec!["wss://r.test".into()])
            .await
            .is_ok());

        // …but this device is no longer that owner.
        sqlx::query("UPDATE settings SET value = 'm-someone-else' WHERE key = ?")
            .bind("readingGroup.memberId")
            .execute(&a.pool)
            .await
            .unwrap();
        let error = set_relays(&a.pool, &group.id, vec!["wss://evil.test".into()])
            .await
            .expect_err("a non-owner is rejected");
        assert!(error.to_string().contains("owner"), "got: {error}");

        // The relay set is untouched by the rejected attempt.
        assert_eq!(
            relays_view(&a.pool, &group.id).await.unwrap().relays,
            vec!["wss://r.test".to_string()]
        );

        cleanup_files(&a.path);
    }
}
