# MyLore — Database Architecture

> Phase 0 · SQLite + FTS5 schema proposal · August 2026
> Companion: `DOMAIN_MODEL.md`, `ARCHITECTURE.md`

---

## 1. Storage Decision

**SQLite via `sqlx` executed in the Rust backend.**

Options evaluated:

| Option | Verdict |
|--------|---------|
| `tauri-plugin-sql` (sqlx wrapper) | Good for simple apps; SQL runs in the webview via IPC, which spreads DB access across the frontend and complicates transactions/repositories and FTS control. **Not chosen as the primary path.** |
| `sqlx` (async, sqlite) directly in Rust with managed state + migrations | Full control: transactions, FTS5, backup via `VACUUM INTO`, WAL, prepared statements, typed rows. **Chosen.** |
| `rusqlite` (sync) | Simpler sync API; we want async on the Tokio runtime. `sqlx` chosen over it. |
| `tauri-plugin-sql` as a *read-only* convenience for the frontend | Rejected: single access path is simpler and safer (one writer). |

Notes (verified 2026): `sqlx` supports SQLite, FTS5 executes as normal SQL, migrations run inside
transactions; Tauri manages the connection pool as app state. Rust >= 1.77.2 toolchain.

Encryption: **not for MVP**. If required later, evaluate `SQLCipher` (needs a bundled/native
sqlite build — a real cost). Revisit as an ADR when demand exists.

---

## 2. Connectivity & PRAGMAs

- `PRAGMA foreign_keys = ON` (enforced per connection).
- `PRAGMA journal_mode = WAL` — concurrent readers, better crash safety.
- `PRAGMA busy_timeout = 5000`.
- `PRAGMA synchronous = NORMAL` (WAL-safe, fast).
- `PRAGMA recursive_triggers = ON` — cascade deletes re-fire child-table triggers, so
  e.g. deleting a `person` refreshes the FTS index even though the `media_person` link is
  removed by the cascade (MISSION-018).
- `PRAGMA integrity_check` at startup (fast) and before/after restore.
- Single writer via a connection pool (sqlx `SqlitePool`, max_connections=1 for writes with
  read replicas via multiple connections — WAL allows many readers).

## 3. Schema (SQL, v1)

Conventions: TEXT UUID PKs, `created_at`/`updated_at` on user-writable aggregates, FK with
`ON DELETE` semantics chosen per aggregate, indexed FK columns, CHECK constraints for enums.

```sql
-- Baseline (migration 0001): app-level metadata.
CREATE TABLE app_meta (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Metadata only. User data lives in separate aggregates (P3).
-- cover_asset_id/banner_asset_id are added by migration 0006 (MISSION-017),
-- once `asset` exists, via ALTER TABLE ADD COLUMN ... REFERENCES asset(id).
-- Enum CHECKs match the migrations.
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
  -- cover_asset_id  TEXT REFERENCES asset(id) ON DELETE SET NULL,  (added in migration 0006)
  -- banner_asset_id TEXT REFERENCES asset(id) ON DELETE SET NULL,  (added in migration 0006)
  provider        TEXT,                      -- provenance of current metadata
  provider_url    TEXT,
  metadata_refreshed_at TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

CREATE TABLE media_alt_title (
  media_id  TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  lang      TEXT NOT NULL,
  title     TEXT NOT NULL,
  PRIMARY KEY (media_id, lang, title)
);

CREATE TABLE person (
  id    TEXT PRIMARY KEY,
  name  TEXT NOT NULL,
  role  TEXT NOT NULL CHECK (role IN
           ('author','artist','director','studio','publisher','network')),
  sort_order INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE media_person (
  media_id   TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  person_id  TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
  PRIMARY KEY (media_id, person_id)
);

CREATE TABLE genre (id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE media_genre (
  media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  genre_id TEXT NOT NULL REFERENCES genre(id) ON DELETE CASCADE,
  PRIMARY KEY (media_id, genre_id)
);

CREATE TABLE tag (
  id        TEXT PRIMARY KEY,
  name      TEXT NOT NULL,
  scope     TEXT NOT NULL CHECK (scope IN ('domain','personal')),
  source    TEXT
);
CREATE TABLE media_tag (
  media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  tag_id   TEXT NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (media_id, tag_id)
);

-- Generic hierarchy tree (DOMAIN_MODEL §2.2)
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
  external_id  TEXT,             -- provider node id (per media, single provider of nodes)
  is_special   INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL
);
CREATE INDEX idx_node_media ON content_node(media_id, kind, position);
CREATE INDEX idx_node_parent ON content_node(parent_id, position);

-- External identity / deduplication (REQ-MEDIA-005)
CREATE TABLE media_external_id (
  media_id  TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  provider  TEXT NOT NULL,
  ext_id    TEXT NOT NULL,
  url       TEXT,
  PRIMARY KEY (provider, ext_id),
  UNIQUE (media_id, provider)
);

-- Media relationships (sequel/prequel/adaptation/...)
CREATE TABLE media_relation (
  from_id   TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  to_id     TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  relation  TEXT NOT NULL CHECK (relation IN
              ('sequel','prequel','adaptation','same_universe','spin_off','other')),
  PRIMARY KEY (from_id, to_id, relation),
  CHECK (from_id <> to_id)
);

-- User-owned aggregates (P3 separation)
CREATE TABLE tracking (
  media_id     TEXT PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE,
  core_status  TEXT NOT NULL CHECK (core_status IN
                 ('planned','in_progress','completed','on_hold','dropped','repeat','wishlist')),
  custom_status_id TEXT REFERENCES status(id) ON DELETE SET NULL,
  started_at   TEXT,
  finished_at  TEXT,
  repeat_count INTEGER NOT NULL DEFAULT 0 CHECK (repeat_count >= 0),
  current_node_id TEXT REFERENCES content_node(id) ON DELETE SET NULL,
  current_position INTEGER,      -- fast "chapter 12" / "ep 5" without node rows
  updated_at   TEXT NOT NULL
);
CREATE INDEX idx_tracking_core_status ON tracking(core_status);
CREATE INDEX idx_tracking_updated_at ON tracking(updated_at);

CREATE TABLE status (
  id       TEXT PRIMARY KEY,
  name     TEXT NOT NULL,
  bucket   TEXT NOT NULL CHECK (bucket IN
             ('planned','in_progress','completed','on_hold','dropped','repeat','wishlist')),
  is_system INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0
);
-- Seeded core statuses (is_system=1): planned, in_progress, completed,
-- on_hold, dropped, repeat, wishlist (MISSION-016).

CREATE TABLE node_progress (
  node_id    TEXT NOT NULL REFERENCES content_node(id) ON DELETE CASCADE,
  state      TEXT NOT NULL CHECK (state IN ('unread','read','watched','skipped','partial')),
  read_at    TEXT,
  note       TEXT,
  rating     INTEGER CHECK (rating BETWEEN 1 AND 10),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (node_id)
);

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

-- provider_setting: not yet assigned to a mission (API keys via OS keyring;
-- created with the provider configuration work, MISSION-063).
CREATE TABLE provider_setting (
  provider  TEXT PRIMARY KEY,
  enabled   INTEGER NOT NULL DEFAULT 1,
  api_key   TEXT,               -- encrypted blob, see ARCHITECTURE §Security
  options   TEXT,               -- JSON
  updated_at TEXT NOT NULL
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Activity log for statistics, calendar, undo (append-only)
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

-- Trash / recovery (REQ-MEDIA-007)
CREATE TABLE trash (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('media','merge','bulk')),
  payload      TEXT NOT NULL,    -- full JSON before-image
  deleted_at   TEXT NOT NULL,
  restored     INTEGER NOT NULL DEFAULT 0 CHECK (restored IN (0, 1))
);
```

### 3.1 Reference-data seeds (migration 0002)

- **genres** (broad categories): action, adventure, comedy, crime, drama, fantasy, historical,
  horror, mystery, psychological, romance, science_fiction, slice_of_life, sports, supernatural,
  thriller, tragedy, war.
- **tags** (domain/community, `scope='domain'`): isekai, reincarnation, time_travel, otome,
  slow_burn, smut, wuxia, xianxia, xuanhuan, cultivation, harem, reverse_harem, shoujo_ai,
  shounen_ai, yuri, yaoi, system, dungeon, academy, game_elements, litrpg.
- Personal tags (`scope='personal'`) are user-created; no seeds.

## 4. Full-Text Search (FTS5)

- **Index (migration 0007, MISSION-018):** two FTS5 virtual tables sharing one assembled
  document per media, `rowid = media rowid`, built from the `v_media_fts_source` view:

```sql
CREATE VIRTUAL TABLE media_fts USING fts5(
  title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids
);
CREATE VIRTUAL TABLE media_fts_cjk USING fts5(
  cjk,
  tokenize = 'trigram'
);
```

- **Stored content, not contentless.** FTS5's special `'delete'` command on a contentless table
  either requires the deleted values (which refresh triggers cannot know) or silently leaves
  orphaned index terms, so a refresh would keep stale matches. Stored content lets triggers use
  plain `DELETE ... WHERE rowid = ?`, which removes the terms correctly (verified 2026).
- **Refresh triggers:** 21 triggers (insert/delete/update on `media`, `media_alt_title`,
  `media_person`, `media_genre`, `media_tag`, `review`, `media_external_id`) delete + reinsert
  the media's document. Cascade deletes re-fire them because connections run with
  `PRAGMA recursive_triggers = ON`.
- **`infrastructure::fts::rebuild`** wipes both tables and repopulates them from the view in one
  transaction — the repair/migration path (bulk import may call it instead of per-row triggers).
- **Tokenization for multilingual titles** (REQ-SEARCH-003):
  - `unicode61` handles Latin + Arabic reasonably (Arabic is space-delimited; we normalize
    diacritics/Alef/Ya/Ta-marbuta before indexing & querying).
  - CJK (ja/zh/ko) needs **`trigram` tokenizer** for substring matching; the `cjk` column is the
    full assembled document, indexed as 3-grams.
  - Arabic folding happens in SQL at index time (view); the **query side must apply the same
    fold** (case-fold, diacritic-fold, script folding) before `MATCH`.
  - Query pipeline: normalize user input the same way, then build FTS5 MATCH (prefix + phrase)
    with `rank` ordering (BM25).

## 5. Indexes & Query Support

- `idx_node_media (media_id, kind, position)` — tree walks, per-type listings (MISSION-014).
- `idx_node_parent (parent_id, position)` — children listings (MISSION-014).
- `tracking(core_status)`, `tracking(updated_at)` — dashboards, "continue" queries.
- `media(content_type)`, `media(title_main)` — library filtering; `title_main` used for the
  non-FTS fast path (startsWith).
- `node_progress` PK on node_id — fast per-node status; aggregate progress is
  `SELECT COUNT(*) FROM node_progress WHERE node_id IN (…chapters…) AND state='read'`.

Cross-row invariants that SQLite cannot express are enforced by Rust validators
(`infrastructure::content_node`, MISSION-014): a node's parent must belong to the same
`media_id`, and the parent chain must stay acyclic.

## 6. Migrations

- Versioned `.sql` files (`migrations/0001_init.sql` … `0007_media_fts.sql`, …) run through
  `sqlx::migrate!` at startup, each **inside a transaction** (verified 2026: sqlx-sqlite wraps
  migration SQL + bookkeeping in a single transaction; see `migrate.rs`). Wired as `db::migrate`
  in MISSION-012.
- Migration 0006 adds `media.cover_asset_id`/`banner_asset_id` via `ALTER TABLE ADD COLUMN`
  after `asset` exists (SQLite rejects writes through an FK pointing at a missing parent, so the
  columns had to wait for the `asset` table).
- Policy:
  - Never edit an applied migration.
  - Backward-compatible only (additive) for app updates; destructive changes happen in the next
    major version behind a backup + explicit confirm.
  - Before any migration run: automatic DB backup to the backup dir (crash/migration-failure
    recovery, REQ-BACKUP-002).
  - After migration: `PRAGMA integrity_check`; on failure, restore the pre-migration backup.
- `schema_version` tracked by sqlx `_sqlx_migrations` table.

## 7. Backup & Restore

- **Backup format:** a single portable `.mylore` archive = SQLite file (via `VACUUM INTO` for a
  consistent online snapshot) + `assets/` manifest + `meta.json` (app version, schema version,
  createdAt, checksums). Zip container.
- **Restore:** validate archive (checksums, `PRAGMA integrity_check`), move current DB aside to
  a `quarantine/` folder (never overwrite silently), open new DB, verify, then delete quarantine
  on success — fully reversible.
- **Automatic:** schedule in preferences (on startup, interval, before migration); rotation keeps
  N latest + one monthly; old backups pruned.
- Import/export of the archive is a first-class UI flow.

## 8. Integrity & Concurrency Summary

- FK enforced (PRAGMA), CHECK constraints, UNIQUE identities.
- Multi-row operations in transactions; batch import in a single transaction with savepoints for
  per-item rollback and a result report.
- WAL + busy_timeout prevent lock contention; a 5s timeout surfaces a typed error, never a hang.
- `updated_at` maintained on aggregates to support future sync conflict policy.
