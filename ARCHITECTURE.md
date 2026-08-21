# MyLore — System Architecture

> Phase 0 · Initial Architecture Proposal · August 2026
> Companion: `DOMAIN_MODEL.md`, `DATABASE.md`, `API_PROVIDERS.md`, `UX_RESEARCH.md`, `DECISIONS.md`

---

## 1. Layering

```
┌────────────────────────────────────────────────────────────────┐
│ PRESENTATION  React SPA (Tauri WebView)                         │
│   components · pages · stores(UI) · i18n · design system        │
│   ✗ NO business logic, ✗ NO raw SQL                             │
├────────────────────────────────────────────────────────────────┤
│ IPC boundary  typed Tauri commands + events (strict TS types)   │
├────────────────────────────────────────────────────────────────┤
│ APPLICATION   Rust · use-cases / services (domain orchestration)│
│   MediaService · TrackingService · ImportService · BackupService│
│   ProviderCoordinator · SearchService · StatsService            │
├────────────────────────────────────────────────────────────────┤
│ DOMAIN        Rust · entities, value objects, invariants        │
│   (pure, no I/O — unit-testable without DB/network)             │
├────────────────────────────────────────────────────────────────┤
│ INFRASTRUCTURE Rust                                             │
│   repositories (sqlx) · SQLite · providers/http (reqwest) ·     │
│   image cache · fs (backups) · keyring · logging                │
└────────────────────────────────────────────────────────────────┘
```

Rules:
- Dependencies point **downward only**. Presentation → Application → Domain ← Infrastructure
  (repositories implement domain ports; domain never imports sqlx/http).
- The DB is replaceable; providers are addable; UI cannot bypass Application.
- No `MediaService.ts` god-files: one Rust module per bounded responsibility, small TypeScript
  modules on the frontend that only translate UI intent into commands.

---

## 2. What lives where (Tauri 2 split)

| Concern | Where | Why |
|---------|-------|-----|
| Business rules, invariants, merges, dedup scoring, stats math | **Rust (Domain/Application)** | testable, fast, single source of truth |
| SQL & transactions | **Rust repositories (sqlx)** | one access path, no SQL in UI |
| Provider HTTP, rate limiting, retries, image downloads | **Rust** | keys never reach the webview; CORS-free; cancelable via Tokio |
| Backup/restore, file archives, migrations | **Rust** | native fs + SQLite control |
| Rendering, layout, animations, forms, search-as-you-type UX | **TS React** | webview is the UI layer |
| UI state (theme, dialogs, routing, selections) | **TS (Zustand)** | ephemeral, not persisted as truth |
| Cached query results | **TS (TanStack Query)** | mirror of domain data shown to user |

Background/long-running work (import, sync, backup) runs as **Tokio tasks** that emit progress
events to the webview (`app.emit("task:progress", …)`); the UI never awaits a blocking command.

## 3. Frontend architecture

**Stack (chosen in `DECISIONS.md`, ADR-003):**
- React 18/19 + Vite + TypeScript strict.
- TanStack Query v5 — remote/domain state over IPC commands (caching, invalidation, optimistic updates).
- Zustand — global *UI* state only (never sync server state into it — two sources of truth).
- React Hook Form + Zod — forms & validation (Add Media, Settings, Import preview, Merge).
- React Router v7 — routing (desktop shell; no SSR concerns).
- TanStack Virtual — virtualization for 10k+ rows.
- Radix UI primitives (headless, RTL-aware) + Tailwind CSS + design tokens (see `DESIGN_SYSTEM.md`).
- i18n: `i18next` (ICU messages, RTL via `dir` attribute + logical CSS props).

**State taxonomy (spec §26):**

| Kind | Tool | Examples |
|------|------|----------|
| UI state | Zustand | selected item, open dialogs, view mode, sidebar, theme |
| Remote/domain state | TanStack Query | library pages, media detail, dashboard widgets |
| Local state | useState | form fields, local toggles |
| Persistent preferences | tauri-plugin-store (JSON) + a settings store | theme, language, defaults, provider prefs |

**Component architecture:** feature folders `features/<feature>/` with `components/`,
`hooks/`, `api.ts` (typed command wrappers), `types.ts`. Shared UI in `components/ui/` from the
design system. Pages compose features.

## 4. Provider architecture (capability-based)

```
Provider interface (Rust trait + TS mirror types):
  search(query) → Candidate[]
  getDetails(id) → MediaMeta
  getNodes(id)   → NodeTree   (episodes/chapters/volumes)
  getRelated(id) → Relation[]
  getReviews(id) → ExternalReview[]   (optional)
  lookupExternalId(id) → ExternalId[]
  enrich(mediaId) → MediaMeta diff

ProviderCapabilities: { search:bool, details:bool, nodes:bool, related:bool,
                        reviews:bool, images:bool, seasonal:bool, auth:'none'|'key'|'oauth' }
```

- Each provider declares capabilities; the app and the UI adapt (a "search only" provider never
  claims to enrich nodes) — spec §44.
- `ProviderCoordinator` handles: routing search queries to all enabled providers for the target
  contentType, rate-limit scheduling, exponential backoff + jitter on 429/5xx, timeouts,
  cancellation tokens, and error mapping into domain errors.
- **Normalization layer:** each adapter maps provider JSON → unified `MediaMeta`/`NodeTree`.
  Domain never sees provider shapes (spec §6). New provider = new adapter crate module, no
  redesign.
- Provider fixtures/recorded responses enable offline tests (`TESTING.md`).

### Provider matrix (verified August 2026 — full detail in `API_PROVIDERS.md`)

| Provider | Content | Auth | Free? | Notes |
|----------|---------|------|-------|-------|
| AniList (GraphQL) | anime+manga | none (public data) | yes | ~90 req/min; rich; external ids incl. MAL/TMDB/AniDB/IMDb |
| Jikan (REST v4) | anime+manga | none | yes | 3 rps / 60 rpm; mirrors MAL |
| MangaDex | manga (incl. manhwa/manhua) | none (public) | yes | chapters, covers, tags; must credit |
| TMDB | movies+TV | free API key | yes (non-commercial) | ~40 req/10s; attribution required |
| TVDB v4 | TV | free key (JWT) | yes | episodes, translations, artwork |
| OpenLibrary | books | none | yes | 1 rps (3 rps with UA+email); works/editions/covers |
| Google Books | books | free key | yes | ~100 req/min/user default |
| Trakt | movies+TV (scrobble) | key; free tier limits | yes (personal) | 2026 free caps: 250 watchlist, 5 lists, 100k history |
| BookBrainz | books (open data) | none | yes | bibliographic + relationships |
| SIMKL | anime+TV+movies | key | yes | aggregate source; has import API |
| Annict | anime (JP) | OAuth | yes | niche; optional |
| NovelUpdates | web novels+light novels | none (HTML scrape) | yes | no API; LNReader-plugin selectors (NU-moderator project); ~1 rps self-throttled; search/details/chapter-tree only (MISSION-065) |
| Hardcover | books (indie) | none (public read) | yes | GraphQL; optional third book provider |
| Bangumi | CN ACGN | none | yes | ~1 rps; optional LN/WN/CN metadata + cross-ids |
| ISBNDB | books (ISBN lookup) | API key | free tier | 100 req/mo; optional paid fallback |

## 5. Search architecture

- **Local:** Rust `SearchService` builds parameterized FTS5 queries (§4 `DATABASE.md`), returns
  ranked results with match snippets. Debounced from the UI (TS).
- **External:** `ProviderCoordinator.searchAll()` runs providers in parallel (bounded), results
  normalized, passed through `IdentityService` to tag results as *"already in library"* /
  *"duplicate candidate"* before display. Combined result model: `{ local: [], external: [] }`.
- Adding an external result → **Import flow**, never a raw insert (dedup, §3 `DOMAIN_MODEL.md`).

## 6. Import / Export pipelines

```
Import:  source(file | provider | manual | backup)
         → Parser → Validator → Normalizer(→ domain model)
         → Deduplicator(identity) → Preview(diff, conflicts)
         → Import transaction(+savepoints) → Result report
```

- Bulk import always shows a **preview** with per-item outcomes (new / duplicate / skipped /
  merge-conflict) and lets the user confirm or cancel (REQ-IMPORT-003).
- Runs as a background task with progress + cancellation; failures are contained per item
  (savepoint rollback) and reported, never aborting the batch silently.
- **Export:** JSON / CSV / Markdown (human-readable) via ExportService; SQLite backup is the
  BackupService path. Export streams to a user-chosen path (dialog) without blocking the UI.

### File import (MISSION-067 → 069)

The pipeline core (`domain::import`, `application::import_pipeline`) is format-agnostic; parsers
(`infrastructure::parsers`) implement the `ImportParser` trait and plug in unchanged.

**MyLore JSON format** (`JsonParser`): a top-level array of item objects. Every field is optional;
counts and years accept numbers **or** strings. Structural problems (not an array, a non-object
element) abort the file with a parse error; content problems are per-row validation issues.

```json
[
  {
    "title": "Sword of the Dawn",
    "title_original": "夜明けの剣",
    "alt_titles": ["Dawn's Sword"],
    "content_type": "novel",
    "format": "light_novel",
    "pub_status": "ongoing",
    "start_date": "2025-01-01",
    "end_date": null,
    "release_year": 2025,
    "language": "ja",
    "country": "JP",
    "content_rating": null,
    "pages": 320,
    "duration_min": null,
    "ep_count": null,
    "ch_count": null,
    "synopsis": "…",
    "people": [{ "role": "author", "name": "Test Author" }],
    "genres": ["Fantasy"],
    "tags": ["isekai"],
    "external_ids": [{ "provider": "anilist", "value": "42", "url": null }],
    "cover_url": "https://…",
    "banner_url": null,
    "my_status": "in_progress",
    "my_rating": 8,
    "my_review": "Lovely.",
    "progress": 12,
    "started_at": "2026-01-05",
    "completed_at": null,
    "repeat_count": 0
  }
]
```

`JsonParser` also reads the MISSION-072 user-state keys (`my_status`, `my_rating`, `my_review`,
`progress`, `started_at`, `completed_at`, `repeat_count`), so an export round-trips the user's list
state as well as the metadata.

**CSV import** (`CsvParser`) reads the file with a user-built `CsvMapping` (the MISSION-068 mapping
UI): each field names a CSV column; fields without a column stay `None` (the validator/normalizer
decide how to degrade the row). `default_content_type` applies one type to every row when the file
has no type column. `delimiter` is the CSV field delimiter, `separator` splits multi-value cells
(`alt_titles`, `genres`, `tags`, `external_id`). External-id cells must be `provider:value`
(e.g. `anilist:42`). Files are parsed `flexible`, so a ragged trailing line never aborts the batch.

- IPC: `import_file_preview` (parse + dedup → preview), `import_commit` (one transaction, savepoint
  per row; `plan` selects rows, null = every `New` row), `import_csv_headers` (column list for the
  mapping pickers), `import_file_detect` (MISSION-072: sniff `json`/`csv`/`anilist`/`goodreads`/
  `storygraph`). The webview picks the file (`<input type="file">` + `FileReader`) and passes the
  text — no fs/dialog plugin needed.
- `application::import_file_service` (`ImportFileService`) routes the five kinds to the right parser,
  detects format from content (MyLore array vs AniList collection for JSON; Goodreads/StoryGraph
  header sniff vs a mapped CSV), and reuses `ImportPipeline` for preview + savepoint commit.
- **Profile exports (MISSION-072):** the **AniList user export** (`AniListParser`), the **Goodreads
  library CSV** (`GoodreadsParser`), and the **StoryGraph CSV** (`StorygraphParser`) read their
  files' built-in, well-known shapes with no mapping UI. Besides metadata they carry the user's list
  state into `ParsedItem`'s `my_*` fields, which `ImportPipeline::insert_row` persists as a
  `tracking` row (status, dates, progress, repeat — normalized in `domain::import`) and a `review`
  row (rating, review) inside the same transaction. Ratings are normalized to the app's 0–10 integer
  scale (AniList score ÷10; Goodreads/StoryGraph ×2); statuses map (AniList CURRENT→in_progress,
  Goodreads `currently-reading`→in_progress, StoryGraph `Did Not Finish`→dropped, …); `repeat_count
  > 0` forces `CoreStatus::Repeat` (tracking invariant). Identity: AniList/MAL ids, Goodreads
  ISBN13 → provider `isbn` + Book Id → `goodreads`. The dialog detects the kind after reading the
  file, shows a profile badge, and skips the mapping table. MAL-via-Jikan and Trakt user lists are
  deliberately out of scope: Jikan has no user-list endpoint and Trakt needs OAuth (API_PROVIDERS.md).
- **Preview + confirm (MISSION-069):** as soon as a file is picked (JSON) or its Title column is
  mapped (CSV), the dialog runs `import_file_preview` automatically and renders the per-item
  outcome list (REQ-IMPORT-003) — a TanStack-Virtual list of `PreviewItem`s with `new` /
  `in_library` / `duplicate` / `invalid` badges and the row's issues. Only `New` rows are
  selectable (default: all); the chosen rows become the `ImportPlan` (`{rows}`) sent to
  `import_commit` on confirm. Cancel closes without writing. The effective CSV mapping sent to
  preview/commit always carries the chosen `delimiter`/`separator`, so a delimiter change
  re-analyzes the file via the `preview(kind, source, mapping)` query key.
- **Background commit (MISSION-070):** confirm spawns the import on the `TaskManager` (ARCHITECTURE
  §8) — `import_commit` resolves with the queued `TaskSnapshot` and the dialog streams `task-changed`
  progress with a cancel button; the report (REQ-IMPORT-004) arrives in the task's typed `result`.
- **Export (MISSION-071/073):** the Settings page's `ExportSection` picks a format (JSON / CSV / Markdown)
  and a destination through the native save dialog (`tauri-plugin-dialog`), then `export_media`
  spawns a `TaskKind::ExportFile` task. `ExportService::stream_to_path` streams rows (title-ordered
  via `media::list_ids`), writing to `<final>.partial` and renaming into place on commit — the
  `PartialExport` guard drops the partial on cancel/error. JSON uses the MISSION-068 import field
  names so an export round-trips back through the importer; **since MISSION-073 the row carries the
  user's list state too** (`progress`, `started_at`, `completed_at` from tracking `finished_at`,
  `repeat_count`), so a JSON export re-imports with status/rating/progress/dates intact. CSV uses a
  fixed 38-column header with `|`-joined multi-values; Markdown renders per-title sections. The
  success `ExportReport {format, total, path}` lands in the task's `result`.
- **Integration tests (MISSION-073):** `tests/import_export.rs` exercises the real pipeline against
  file-backed migrated DBs (`db::init`) using fixtures under `tests/fixtures/import/` (which also
  serve as sample files for the Import dialog). `infrastructure::test_support` is `#[cfg(test)]`-gated,
  so the integration tests define their own small helpers.
- **Review & notes (MISSION-074):** the detail page's Review tab edits the same `review` row the import
  writes. `commands/review.rs` exposes `review_get`/`review_save`/`review_delete` backed by
  `ReviewService` — `save` validates the media exists and the domain invariants (1–10 rating, spoiler
  flag requires text), preserves `created_at`, stamps `updated_at`, treats an entirely empty review as
  a delete, and logs a `reviewed` activity row. `commands/media.rs` adds single-media personal-tag
  commands (`media_tags`/`media_add_tag`/`media_remove_tag`) that resolve-or-create `tag-{uuid}`
  rows in the personal scope only (domain tags never surface). Frontend: `ReviewTab` + hooks in
  `features/library/review.ts`, i18n `review.*` EN/AR.
- **Favorites flag (MISSION-075):** `MediaListItem` carries the `review.favorite` flag (the repo's
  `media::list`/search/dashboard queries already selected `COALESCE(r.favorite, 0)`), and the grid /
  list / compact views render the shared `FavoriteFlag` heart on favorited rows. The favorites filter
  shipped earlier with the filter panel (MISSION-041) — it maps to the SQL `r.favorite = 1` predicate.
- **Collections (MISSION-076):** manual collections live in the M6 `collection` /
  `collection_member` tables. `commands/collection.rs` exposes 8 commands
  (`collection_list`/`create`/`rename`/`delete`/`members`/`bulk_add`/`remove_member`/`reorder`) backed
  by `CollectionService` — the ordering column is `collection_member.position`, and membership reads
  join `collection_member × media × LEFT JOIN review` so the detail rows carry progress + the
  `favorite` flag. **`collection_reorder` validates the given media-id set equals the current members**
  (guards against drift) and rewrites positions 0..n preserving `added_at`. The frontend Collections
  page (`/collections`) is a card grid with create/rename/delete dialogs; the detail page
  (`/collections/:id`) reorders members with **native HTML5 drag-and-drop** (no DnD library — handlers
  never read `dataTransfer`, so jsdom tests drive them via `fireEvent`) plus accessible Up/Down
  buttons, with optimistic cache writes that roll back on error. Bulk add to a collection from the
  library action bar now routes through `collection_bulk_add` (the MISSION-045 `BulkService`
  collection path was retired).
- **Smart collections (MISSION-077):** a collection is *computed* instead of manual when
  `collection.is_smart = 1` — `collection.filter_def` holds a JSON `SmartFilter`
  (content_type/format/pub_status/genre/tag/year/favorite + sort/ascending, all nullable, mirroring
  `LibraryFilters` + `LibrarySort`). `CollectionService::create_smart`/`update_smart_filter`
  (smart-only) persist the filter; `members()` **routes server-side** — a smart collection re-runs
  `media_repo::list` (sort/ascending resolved exactly like `media_service`) and batches through
  `MediaService::to_list_items`, so the frontend uses the same members query key with no conditional
  hooks; `list()`/`view()` compute the smart `member_count` via `media_repo::count`. Manual membership
  ops (`add_members`/`remove_member`/`reorder`) reject smart collections. Frontend: `SmartFilterForm`
  is the query builder (facet selects fed by `media_facets`), the library toolbar's **"Save as
  collection"** snapshots the active filter + sort into `collection_create_smart`, the Collections
  page shows a Smart badge + create-smart dialog, and the detail page renders computed members
  read-only with an "Edit filter" dialog.
- **Bulk ops (MISSION-078):** the library action-bar actions (`tracking_bulk_set_status`,
  `media_bulk_add_tag`, `media_bulk_delete`, `collection_bulk_add`) accept an optional `BulkFilter`
  (the 7 library facets). When present, `resolve_targets` maps it through the same `media_repo::list`
  query and the operation runs against the **whole filtered selection** server-side (the client's
  `ids` are ignored), so "apply to all N matching" never ships a giant id array over IPC. Every
  operation resolves with a **per-item `BulkResult`** (`total`/`succeeded`/`failed` + `failures`
  `{media_id, reason}`) — a media that can't move to the target status, an unknown id, or a FK
  violation is recorded instead of aborting the batch; `media_bulk_delete` returns a `BulkDeleteResult`
  whose `trash_ids` cover exactly the successful deletions so group undo is precise. The action bar
  surfaces this with a **scope toggle** ("Selected N" / "All N matching") and summary toasts.
- **Review metadata (MISSION-079):** StoryGraph-style mood / pace / content-warning fields live on the
  `review` row (`migration 0009` — `moods`/`content_warnings` as canonical JSON arrays, `pace` as a
  CHECK-constrained single value, `warnings_acknowledged_at`). The domain owns the fixed
  vocabularies (`Mood`/`Pace`/`ContentWarning` in `domain/review.rs`); `ReviewService::save`
  normalizes keys (vocabulary-validated, sorted, deduped) and treats the acknowledgment as metadata
  of the *current* warning set — **preserved when the set is unchanged, cleared when it changes or
  becomes empty**, never forced. `review_acknowledge_warnings` stamps `warnings_acknowledged_at` now
  (idempotent, requires a review with warnings). The detail-page hero renders the badges from the
  review row (reusing the shared `review.forMedia` cache) with a one-tap acknowledge; the Review tab
  edits them via chips. The `pace` column's CHECK stays server-side only (the vocabulary validation
  is duplicated in `features/library/reviewMeta.ts` for the pickers).
- **Stats (MISSION-080):** `StatsService::summary` wires the MISSION-027 pure `compute_stats`
  (`domain/stats.rs`) to the DB: `tracking_repo::tracked_media` (tracking JOIN media LEFT JOIN review —
  one row per tracked title carrying status, content type, rating, favorite, release year) plus
  `tracking_repo::progress_stats` (batched aggregates over countable node kinds; book chapters are
  weighed by page count and consumed episode minutes are summed into `consumed_hours`). Stats are
  **real-data-only** — no `with_estimate` node-tree totals enter the picture. The `StatsView` DTO
  ships counts + distributions (keys are the enum strings reused for `coreStatus.*` / `contentType.*`
  i18n) over `stats_summary`; the Stats page renders seven tabular-numbers cards and four
  hand-rolled horizontal-bar charts (no chart library, logical properties keep it RTL-safe).
- **Recap (MISSION-082):** `RecapService::year` turns a year of activity (MISSION-051) into a
  celebratory recap. It reuses `calendar::activity_in_range` for the raw events (same projection),
  filters them to the *local* year (each RFC3339 timestamp converted via `chrono::Local`, queried
  with the same ±1-day window), and derives `RecapTotals` (distinct media per kind + progress-event
  count), `by_month` completions, `best_month`, `top_media` (top 5 by event count) and
  `longest_streak` (longest run of consecutive active days) in one pass. `recap_repo::completed_genres`
  ranks the finished media's genres by distinct-media count. `YearRecap` ships over `recap_year`; the
  Recap page renders stat cards, highlights, a hand-rolled 12-bar month chart (`Intl` month names,
  best month in accent) and the standouts list.
- **Calendar (MISSION-081):** `CalendarService::month` assembles one month of **air dates** and
  **activity** per local calendar day. Air events come from `content_node.release_date` (direct
  `[start, next_month)` window) LEFT JOINed with media for title/content_type; activity events are
  bucketed by converting each RFC3339 `created_at` to `chrono::Local` and querying a **±1-day
  window**, so any event whose *local* date falls in the month is captured regardless of UTC offset.
  `calendar_repo::air_dates` / `activity_in_range` return `AirDateRow`/`ActivityRow` (title already
  joined); the service shapes `CalendarItem` (label via the shared `unit_label` — "E5"/"Ch7") and
  `CalendarDay` (no `today` — the frontend computes its own local today). `CalendarPage` renders a
  Sunday-start 7-column grid with `aria-pressed` day cells (accent dot = air, tertiary dot =
  activity), RTL-aware chevrons, and a day-list panel whose chips link to `library/:id`.
- **Reading recap (MISSION-083):** `ReadingRecapService::recap(year)` adds StoryGraph-style reading
  stats under the Stats page. `reading_repo::monthly_reading` scans consumed nodes in a ±1-day
  local-year window (state read/watched, content types book/novel/web_novel/manga/manhwa/manhua);
  book chapters weigh by `page_count` (1 when unknown), non-book chapters count as chapters but
  0 pages — identical to MISSION-080's `consumed_pages`. Distinct finished media reuse
  `activity_in_range` (kind completed + reading type + local year). All-time taste distributions:
  moods/pace folded from the review rows' JSON metadata (`taste_rows`), formats from tracked
  reading media (`reading_formats`). Migration `0012` is index-only (`node_progress(read_at)`).
  `ReadingRecap` ships over `reading_recap`; the Stats page renders the **ReadingSection** (year
  select, three tabular cards, two hand-rolled month-bar charts and three horizontal distribution
  charts reusing the shared `DistributionChart`, extracted from StatsPage for reuse).
- **Reading groups (MISSION-114–118, planned seam):** decentralized friend groups for tracking
  novels/books together — local-first, no central server. Design constraints fixed up front:
  group data lives in its own tables (`reading_group` / `group_member` / `group_note`), never in
  the personal aggregates ADR-007 protects; cross-device references use a stable *work identity*
  (provider id or normalized title+author+year hash via the existing identity_candidates logic),
  since each device's `media` UUIDs differ. Conflict policy by ownership: CRDT (`yrs`) only for
  shared notes; each member's shelf is single-writer; group settings owner-only with an epoch.
  Transport is async store-and-forward over Nostr relays behind a `p2p` cargo feature (default
  build stays dependency-free), outbox-first so nothing is lost offline.
  - **Threat model (explicit):** E2EE (XChaCha20-Poly1305, group key in the OS keyring, shared
    only via out-of-band QR/link invite) protects payloads, but public relays still observe
    metadata — IP address, pubkey, timing, packet sizes, group size. The feature is therefore
    fully opt-in behind an explicit privacy screen stating what leaves the device, ships relay +
    E2EE status badges, supports self-hosted relays, avoids presence indicators entirely, and
    rotates the group key (new epoch) when a member is removed.

## 7. Backup & Restore

`BackupService` in Rust: snapshot (VACUUM INTO) + assets manifest + meta.json → `.mylore` zip.
Restore validates, quarantines current data, swaps, verifies. Automatic scheduling + rotation.

**Shipped (MISSION-084/085):** `BackupService::create` packs a consistent `VACUUM INTO` snapshot of
the live WAL database, every cached asset file, and a `meta.json` manifest (format version,
counts, asset id → archive-path map) into `{data_dir}/backups/mylore-<stamp>-<id>.mylore` — a
plain deflate zip. Writes go to a `.partial` sibling renamed into place on success; a drop guard
removes the partial and the temp snapshot on failure or cancellation. Every archive is
re-opened and validated before success is reported: manifest parses at the current format
version, the embedded snapshot passes `PRAGMA integrity_check`, and its media count matches the
manifest. `backup_create` runs as a cancelable `TaskKind::Backup` task on the TaskManager
(`BackupReport` as the typed result); `backup_validate(path)` validates any archive on demand.

**Restore (MISSION-085):** `BackupService::restore(path)` validates with zero side effects,
stages the archive's contents, closes the live pool (the DB files are locked on Windows while
open), quarantines the current database + images under `{data_dir}/quarantine-…` (original
names, so rollback is a plain move back), swaps the restored data into place, repoints each
manifest asset's `local_path` at its restored file, and verifies the result — rolling back on
any failure. There is no cancellation checkpoint after validation (a dropped future mid-swap
would skip the rollback). The restore closes the managed pool, so it reports
`restart_required: true` and the UI restarts the app (`TaskKind::Restore`, not cancelable by
design). Scheduling + rotation (086) and the UI (088) build on this format.

**Schedule + rotation (MISSION-086):** backup preferences (`BackupPrefs` — auto on/off,
interval hours, keep count) live in the `settings` table under `backup.*` keys with safe
defaults (off / 24h / 10). Every `create()` applies the retention policy afterwards:
`rotate(keep)` keeps the newest N archives plus the newest of each older month ("N + monthly"),
sorting by the stamp embedded in the file name and never touching foreign files. At startup the
app checks ~20s in whether an automatic backup is due (enabled + newest archive older than the
interval) and creates one directly — logged, not routed through the task list.
(Full design: `DATABASE.md §7`.)

## 8. Background tasks

Unified `TaskManager` (**implemented**): every long operation (import, export, metadata sync, image
download, backup, migration, provider search) is a cancelable task with states
`queued → running(p) → success|failed|cancelled`, progress events to the UI, and a typed result.
`TaskManager` is managed as `Arc<TaskManager>`; its emitter forwards every change as a `task-changed`
event (payload `TaskSnapshot`). The import confirm command spawns a `TaskKind::ImportFile` task that
runs `ImportPipeline::commit_with_progress` inside `tokio::select!` against a cooperative cancel
flag; the export command spawns a `TaskKind::ExportFile` task that streams rows the same way.
Cancellation propagates to Tokio tasks and HTTP requests (drop-based cancellation).

- `domain::task::TaskSnapshot` — id, kind, title, state, `progress: Option<u32>`, message, error,
  `result: Option<Value>` (the typed outcome, e.g. the `ImportReport` on a successful import).
- `application::task_service::{TaskManager, TaskReporter}` — spawn/get/list/cancel; the reporter's
  watch channel backs `cancel` and progress.
- `commands::tasks::{task_list, task_get, task_cancel}` (+ the `task-changed` event via codegen).
- Frontend: `useImportTask(taskId)` subscribes to `task-changed` per task, falls back to `task_get`,
  and invalidates library queries on success; `useTaskCancel` requests cancellation. `ImportFileDialog`
  streams progress with a cancel button and shows the report from the task's `result`.

## 9. Error handling

- **Domain errors** (`MediaNotFound`, `InvalidStatus`, `DuplicateConflict`) — typed enums in Rust.
- **Infrastructure errors** (`DbError`, `IoError`, `BackupCorrupt`).
- **Provider errors** (`RateLimited{retryAfter}`, `Timeout`, `ProviderDown`, `NotFound`,
  `AuthFailed`, `NormalizationError`).
- Commands return `Result<T, AppError>`; the TS wrapper maps `AppError` to typed JS errors with
  user-facing messages + codes. No empty `catch {}` anywhere (spec §42).
- UI error surfaces: inline field errors, toast for transient, dedicated retry/empty states.

## 10. Logging

- Rust `tracing` → rolling files under app-data `logs/`, max size + rotation, no secrets
  (API keys redacted by type — provider settings never logged).
- Levels: error/warn by default; `debug` toggle in settings; stdout mirror in dev.
- Frontend console errors forwarded to the same file in dev; release keeps a minimal logger.

## 11. Security

- **Capabilities (least privilege):** custom commands only; do **not** expose
  `fs:allow-*`/`shell:allow-*`/`http:allow-*` globals to the webview. Dialog plugin scoped to
  pick paths; HTTP happens in Rust. CSP in `tauri.conf.json`; no remote origins; no devtools in
  release.
- **API keys:** stored via OS keyring (`keyring` crate), encrypted fallback file with a
  per-machine key; never in git, never in logs (spec §46).
- IPC commands validate/parse all inputs; SQL via bind params only.
- Shell: only an explicitly scoped "open external link" action via `shell` open API for
  provider URLs, with allowlist of schemes.

## 12. Performance

- Startup: minimal bootstrap → render shell → lazy-load data queries (spec §100).
- Virtual lists (10k+), windowed queries, debounced search, image cache on disk + memoized
  components; indexes per `DATABASE.md §5`.
- Benchmarks in CI: search latency, insert, bulk import, startup, memory (spec §99).
- No premature optimization; measure before and after.

## 13. i18n & RTL

- `i18next` + ICU; locales `en` and `ar` in MVP, provider names/titles kept in original script.
- Layout via **logical CSS properties** and Tailwind `rtl:` variants — mirrored navigation,
  sidebars, and text alignment work without separate stylesheets.
- Design tokens and components tested in both directions and with long CJK/Arabic titles
  (REQ-UX-003, NFR-I18N).

## 14. Accessibility

Keyboard-first, focus management per route, ARIA via Radix, WCAG 2.1 AA contrast in both themes,
`prefers-reduced-motion` support, screen-reader labels on all icons/actions (REQ-UX-004).
