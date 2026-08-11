-- MISSION-013: media aggregates + reference-data seeds.
-- Media metadata, alternative titles, people (authors/artists/studios/...),
-- genres and domain/community tags, with many-to-many joins.
--
-- `cover_asset_id`/`banner_asset_id` are NOT defined here: they reference
-- `asset(id)`, which is created in MISSION-017. SQLite would refuse inserts
-- into a table whose FK points at a missing parent, so the columns are added
-- there via `ALTER TABLE ADD COLUMN ... REFERENCES asset(id)`.

CREATE TABLE media (
  id              TEXT PRIMARY KEY,          -- UUID
  content_type    TEXT NOT NULL CHECK (content_type IN
                    ('book','novel','web_novel','manga','manhwa','manhua','anime','tv','movie','other')),
  format          TEXT,                      -- optional refinement (light_novel, webtoon, ova, ...)
  title_main      TEXT NOT NULL,
  title_original  TEXT,
  synopsis        TEXT,
  pub_status      TEXT NOT NULL DEFAULT 'unknown' CHECK (pub_status IN
                    ('announced','ongoing','completed','hiatus','cancelled','unknown')),
  start_date      TEXT,                      -- ISO date
  end_date        TEXT,
  release_year    INTEGER,
  language        TEXT,
  country         TEXT,
  content_rating  TEXT,
  pages           INTEGER,                   -- optional aggregate
  duration_min    INTEGER,
  ep_count        INTEGER,                   -- estimate/known count
  ch_count        INTEGER,
  -- cover_asset_id/banner_asset_id added with asset (MISSION-017)
  provider        TEXT,                      -- provenance of current metadata
  provider_url    TEXT,
  metadata_refreshed_at TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);
CREATE INDEX idx_media_content_type ON media(content_type);
CREATE INDEX idx_media_title_main ON media(title_main);

CREATE TABLE media_alt_title (
  media_id  TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  lang      TEXT NOT NULL,
  title     TEXT NOT NULL,
  PRIMARY KEY (media_id, lang, title)
);

CREATE TABLE person (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  role       TEXT NOT NULL CHECK (role IN
               ('author','artist','director','studio','publisher','network')),
  sort_order INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE media_person (
  media_id   TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  person_id  TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
  PRIMARY KEY (media_id, person_id)
);

CREATE TABLE genre (
  id   TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);
CREATE TABLE media_genre (
  media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  genre_id TEXT NOT NULL REFERENCES genre(id) ON DELETE CASCADE,
  PRIMARY KEY (media_id, genre_id)
);

CREATE TABLE tag (
  id     TEXT PRIMARY KEY,
  name   TEXT NOT NULL,
  scope  TEXT NOT NULL CHECK (scope IN ('domain','personal')),
  source TEXT
);
CREATE TABLE media_tag (
  media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  tag_id   TEXT NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (media_id, tag_id)
);

-- Seeds: core genres (broad categories) and domain tags (community/domain
-- conventions; see DOMAIN_MODEL §2.6 and RESEARCH notes).
INSERT INTO genre (id, name) VALUES
  ('action', 'Action'),
  ('adventure', 'Adventure'),
  ('comedy', 'Comedy'),
  ('crime', 'Crime'),
  ('drama', 'Drama'),
  ('fantasy', 'Fantasy'),
  ('historical', 'Historical'),
  ('horror', 'Horror'),
  ('mystery', 'Mystery'),
  ('psychological', 'Psychological'),
  ('romance', 'Romance'),
  ('science_fiction', 'Science Fiction'),
  ('slice_of_life', 'Slice of Life'),
  ('sports', 'Sports'),
  ('supernatural', 'Supernatural'),
  ('thriller', 'Thriller'),
  ('tragedy', 'Tragedy'),
  ('war', 'War');

INSERT INTO tag (id, name, scope) VALUES
  ('isekai', 'Isekai', 'domain'),
  ('reincarnation', 'Reincarnation', 'domain'),
  ('time_travel', 'Time Travel', 'domain'),
  ('otome', 'Otome', 'domain'),
  ('slow_burn', 'Slow Burn', 'domain'),
  ('smut', 'Smut', 'domain'),
  ('wuxia', 'Wuxia', 'domain'),
  ('xianxia', 'Xianxia', 'domain'),
  ('xuanhuan', 'Xuanhuan', 'domain'),
  ('cultivation', 'Cultivation', 'domain'),
  ('harem', 'Harem', 'domain'),
  ('reverse_harem', 'Reverse Harem', 'domain'),
  ('shoujo_ai', 'Shoujo Ai', 'domain'),
  ('shounen_ai', 'Shounen Ai', 'domain'),
  ('yuri', 'Yuri', 'domain'),
  ('yaoi', 'Yaoi', 'domain'),
  ('system', 'System', 'domain'),
  ('dungeon', 'Dungeon', 'domain'),
  ('academy', 'Academy', 'domain'),
  ('game_elements', 'Game Elements', 'domain'),
  ('litrpg', 'LitRPG', 'domain');
