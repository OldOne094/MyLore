-- MISSION-115: reading-groups CRDT document store (behind the `p2p` feature).
--
-- One years/yrs document per (group, work) holds the shared notes as a
-- conflict-free map (note id → body). `state` is the encoded document;
-- `pending_ops` counts updates applied since the last compaction so the engine
-- can snapshot on a size/time policy. The plain `group_note` table from
-- MISSION-114 stays the non-p2p source of truth; this document is additive.

CREATE TABLE group_doc (
  group_id     TEXT NOT NULL REFERENCES reading_group(id) ON DELETE CASCADE,
  work_key     TEXT NOT NULL,
  state        BLOB NOT NULL,
  pending_ops  INTEGER NOT NULL DEFAULT 0,
  compacted_at TEXT,
  updated_at   TEXT NOT NULL,
  PRIMARY KEY (group_id, work_key)
);
