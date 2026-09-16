-- MISSION-114: decentralized reading groups (local model).
--
-- Group data is a separate aggregate (ADR-007): it never joins into the
-- personal aggregates (tracking / review / collections). A group references a
-- "work" by a stable cross-device key (provider id or a normalized
-- title+author+year hash), never by a device-local media UUID — see
-- `domain::reading_group::work_key`.
--
-- `epoch` is a forward seam: MISSION-118 rotates the group key (and bumps the
-- epoch) when a member is removed; MISSION-115/116 add the CRDT/E2EE payloads
-- and the Nostr transport on top of these tables. `group_note.body` is plain
-- text here and becomes the CRDT-backed document in MISSION-115.

CREATE TABLE reading_group (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  owner_id   TEXT NOT NULL,
  epoch      INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE group_member (
  group_id     TEXT NOT NULL REFERENCES reading_group(id) ON DELETE CASCADE,
  member_id    TEXT NOT NULL,
  display_name TEXT NOT NULL,
  role         TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('owner','member')),
  joined_at    TEXT NOT NULL,
  PRIMARY KEY (group_id, member_id)
);

-- One member's shelf entry for a work (single-writer per member).
CREATE TABLE group_shelf (
  group_id     TEXT NOT NULL,
  member_id    TEXT NOT NULL,
  work_key     TEXT NOT NULL,
  title        TEXT NOT NULL,
  content_type TEXT NOT NULL,
  status       TEXT NOT NULL,
  progress     INTEGER NOT NULL DEFAULT 0 CHECK (progress >= 0),
  updated_at   TEXT NOT NULL,
  PRIMARY KEY (group_id, member_id, work_key),
  FOREIGN KEY (group_id, member_id) REFERENCES group_member(group_id, member_id) ON DELETE CASCADE
);
CREATE INDEX idx_group_shelf_work ON group_shelf(group_id, work_key);

-- Shared notes per work (the CRDT-backed document lands in MISSION-115).
CREATE TABLE group_note (
  id         TEXT PRIMARY KEY,
  group_id   TEXT NOT NULL REFERENCES reading_group(id) ON DELETE CASCADE,
  work_key   TEXT NOT NULL,
  author_id  TEXT NOT NULL,
  body       TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_group_note_work ON group_note(group_id, work_key, created_at);
