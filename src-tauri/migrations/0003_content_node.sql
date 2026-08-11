-- MISSION-014: generic content-node tree (DOMAIN_MODEL §2.2).
-- One node type models every hierarchy and flat media: seasons→episodes for
-- TV, volumes→chapters for manga/novels, page ranges, tracks, issues, etc.
--
-- `parent_id` self-reference is a plain FK; the cross-row invariant that a
-- parent belongs to the same `media_id` (and that the tree stays acyclic)
-- cannot be expressed in SQL and is enforced by the Rust validators in
-- `infrastructure::content_node`.

CREATE TABLE content_node (
  id           TEXT PRIMARY KEY,
  media_id     TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  parent_id    TEXT REFERENCES content_node(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN
                 ('season','episode','volume','chapter','page_range','track','issue','node')),
  position     INTEGER NOT NULL,
  number       TEXT,             -- display number (e.g. "12.5")
  title        TEXT,
  release_date TEXT,
  duration_min INTEGER,
  page_count   INTEGER,
  synopsis     TEXT,
  external_id  TEXT,             -- provider node id (single provider of nodes per media)
  is_special   INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL
);

CREATE INDEX idx_node_media ON content_node(media_id, kind, position);
CREATE INDEX idx_node_parent ON content_node(parent_id, position);
