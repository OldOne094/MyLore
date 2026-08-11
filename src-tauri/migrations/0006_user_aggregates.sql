-- MISSION-017: user-owned aggregates + asset + activity/trash/settings.
--
-- This migration also adds the deferred `media.cover_asset_id` /
-- `media.banner_asset_id` columns (see 0002_media.sql). SQLite supports
-- `ALTER TABLE ADD COLUMN ... REFERENCES asset(id)` here because the column
-- has no default (implicitly NULL for existing rows).
--
-- `provider_setting` (API keys via keyring) is created with the provider
-- configuration work (MISSION-063), not here.

CREATE TABLE review (
  media_id     TEXT PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE,
  rating       INTEGER CHECK (rating BETWEEN 1 AND 10),
  review       TEXT,
  short_review TEXT,
  notes        TEXT,
  favorite     INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
  is_spoiler   INTEGER NOT NULL DEFAULT 0 CHECK (is_spoiler IN (0, 1)),
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);

CREATE TABLE collection (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  is_smart   INTEGER NOT NULL DEFAULT 0 CHECK (is_smart IN (0, 1)),
  filter_def TEXT,               -- JSON filter definition for smart collections
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
CREATE TABLE collection_member (
  collection_id TEXT NOT NULL REFERENCES collection(id) ON DELETE CASCADE,
  media_id      TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  position      INTEGER NOT NULL DEFAULT 0,
  added_at      TEXT NOT NULL,
  PRIMARY KEY (collection_id, media_id)
);

CREATE TABLE asset (
  id            TEXT PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('cover','banner','avatar','node_image')),
  remote_url    TEXT,
  local_path    TEXT,
  status        TEXT NOT NULL DEFAULT 'remote' CHECK (status IN ('remote','cached','failed','missing')),
  mime_type     TEXT,
  width         INTEGER,
  height        INTEGER,
  etag          TEXT,
  last_fetched_at TEXT,
  created_at    TEXT NOT NULL
);

ALTER TABLE media ADD COLUMN cover_asset_id TEXT REFERENCES asset(id) ON DELETE SET NULL;
ALTER TABLE media ADD COLUMN banner_asset_id TEXT REFERENCES asset(id) ON DELETE SET NULL;

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE activity (
  id         TEXT PRIMARY KEY,
  media_id   TEXT REFERENCES media(id) ON DELETE CASCADE,
  node_id    TEXT REFERENCES content_node(id) ON DELETE CASCADE,
  kind       TEXT NOT NULL CHECK (kind IN
               ('added','started','progress','completed','repeat','reviewed','deleted','merged','imported')),
  meta       TEXT,               -- JSON
  created_at TEXT NOT NULL
);
CREATE INDEX idx_activity_media ON activity(media_id, created_at);

CREATE TABLE trash (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('media','merge','bulk')),
  payload      TEXT NOT NULL,    -- full JSON before-image
  deleted_at   TEXT NOT NULL,
  restored     INTEGER NOT NULL DEFAULT 0 CHECK (restored IN (0, 1))
);
