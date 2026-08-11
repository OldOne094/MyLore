-- MISSION-016: tracking, seeded core statuses, node progress.
--
-- `status` carries the core status set (seeded, is_system=1) plus user custom
-- statuses (is_system=0) grouped under a core bucket. `tracking` stores the
-- per-media user state; `core_status` is validated against the core set by
-- CHECK (MISSION-024 builds the full status engine on top). `node_progress`
-- records per-node state; aggregate progress is derived, never stored.

CREATE TABLE status (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  bucket     TEXT NOT NULL CHECK (bucket IN
               ('planned','in_progress','completed','on_hold','dropped','repeat','wishlist')),
  is_system  INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0
);

INSERT INTO status (id, name, bucket, is_system, sort_order) VALUES
  ('planned',     'Planned',     'planned',     1, 10),
  ('in_progress', 'In Progress', 'in_progress', 1, 20),
  ('completed',   'Completed',   'completed',   1, 30),
  ('on_hold',     'On Hold',     'on_hold',     1, 40),
  ('dropped',     'Dropped',     'dropped',     1, 50),
  ('repeat',      'Repeat',      'repeat',      1, 60),
  ('wishlist',    'Wishlist',    'wishlist',    1, 70);

CREATE TABLE tracking (
  media_id         TEXT PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE,
  core_status      TEXT NOT NULL CHECK (core_status IN
                     ('planned','in_progress','completed','on_hold','dropped','repeat','wishlist')),
  custom_status_id TEXT REFERENCES status(id) ON DELETE SET NULL,
  started_at       TEXT,
  finished_at      TEXT,
  repeat_count     INTEGER NOT NULL DEFAULT 0 CHECK (repeat_count >= 0),
  current_node_id  TEXT REFERENCES content_node(id) ON DELETE SET NULL,
  current_position INTEGER,        -- fast "chapter 12" / "ep 5" without node rows
  updated_at       TEXT NOT NULL
);

CREATE INDEX idx_tracking_core_status ON tracking(core_status);
CREATE INDEX idx_tracking_updated_at ON tracking(updated_at);

CREATE TABLE node_progress (
  node_id    TEXT NOT NULL REFERENCES content_node(id) ON DELETE CASCADE,
  state      TEXT NOT NULL CHECK (state IN ('unread','read','watched','skipped','partial')),
  read_at    TEXT,
  note       TEXT,
  rating     INTEGER CHECK (rating BETWEEN 1 AND 10),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (node_id)
);
