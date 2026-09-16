-- MISSION-116: reading-groups relay transport (behind the `p2p` feature).
--
-- Outbox-first: a sealed envelope is written here *before* it is broadcast, so
-- a crash or an offline window never loses a change — the next flush retries.
-- `group_event_seen` makes delivery idempotent: an event id is merged at most
-- once, no matter how many relays hand it back.
--
-- Chunking (large envelopes split into <=64KB events) lives in the transport
-- layer, not here: this table stores one whole sealed envelope per row.

CREATE TABLE group_relay (
  group_id TEXT NOT NULL REFERENCES reading_group(id) ON DELETE CASCADE,
  url      TEXT NOT NULL,
  PRIMARY KEY (group_id, url)
);

CREATE TABLE group_outbox (
  id         TEXT PRIMARY KEY,
  group_id   TEXT NOT NULL REFERENCES reading_group(id) ON DELETE CASCADE,
  work_key   TEXT NOT NULL,
  topic      TEXT NOT NULL,
  message_id TEXT NOT NULL,
  payload    BLOB NOT NULL,
  created_at TEXT NOT NULL,
  sent_at    TEXT,
  attempts   INTEGER NOT NULL DEFAULT 0,
  last_error TEXT
);

CREATE INDEX idx_group_outbox_pending ON group_outbox (group_id, sent_at);

CREATE TABLE group_event_seen (
  event_id    TEXT PRIMARY KEY,
  group_id    TEXT NOT NULL,
  topic       TEXT NOT NULL,
  received_at TEXT NOT NULL
);
