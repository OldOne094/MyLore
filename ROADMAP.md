# MyLore — Master Roadmap

> Phase 0 · Master roadmap · August 2026
> **This is the single source of truth for what we build and in what order.**
> Milestones first (§2), then the complete mission list (§3).
> Detailed per-mission implementation notes, files, tests and acceptance criteria stay in
> `DEVELOPMENT_PLAN.md` (the reference). Requirements in `PROJECT_REQUIREMENTS.md`.

---

## 1. Product in one line

A **local-first, offline-first, private desktop media tracker** (novels, web novels, light
novels, books, manga/manhwa/manhua, anime, TV, movies) — single binary (Tauri 2), own SQLite
database, no account, no ads, no telemetry. The internet is only a metadata tap; the library is
the source of truth. (`PROJECT_REQUIREMENTS.md`, `PHASE0_REPORT.md`.)

**Missions** are the smallest actionable units. Each mission has: ID · Title · Goal · Deps ·
Files · Tests · Acceptance Criteria · Complexity (S/M/L) · Priority · Status. When picked up, its
full checklist is created in the epic folder (`DEVELOPMENT_PLAN.md §5`).

---

## 2. Milestones

Milestones are the phases. We never build ahead of a milestone's exit criteria (spec §61).

| MS | Name | Exit criterion | Deps |
|----|------|----------------|------|
| M0 | Research & Design | This document set (Phase 0) incl. roadmap. | — |
| M1 | Foundation | Tauri 2 app builds on Win/macOS/Linux; TS strict + lint + fmt + CI green; typed IPC skeleton; empty window shell. | M0 |
| M2 | Database | SQLite via sqlx: migrations, full schema, repositories, FTS5, integrity pragmas; tests green. | M1 |
| M3 | Domain Layer | Domain entities/services in Rust with unit tests (tracking math, dedup, status transitions, stats, merge). | M2 |
| M4 | UI Foundation | Design tokens, themes, router shell, layout + nav rail, i18n (en/ar, RTL), command palette skeleton. | M1–M3 |
| M5 | Library MVP | Manual add, media CRUD, library grid/list/compact, filters + sort, media detail page, trash/undo. | M2–M4 |
| M6 | Tracking | Node trees, per-node progress (incl. novel chapter read-state), Normal/Manual mode, quick capture, status transitions, dashboard widgets. | M5 |
| M7 | Providers | Coordinator + AniList/TMDB/MangaDex/OpenLibrary (+ Jikan/Google fallback); external search; enrich; identity/dedup; optional Hardcover/Bangumi. | M3, M5 |
| M8 | Import/Export | Import pipeline + preview + reports; JSON/CSV/Markdown export; Goodreads/StoryGraph CSV + AniList/MAL imports. | M7 |
| M9 | Reviews & Collections | Reviews/notes/tags, favorites, collections + smart lists, bulk operations, content warnings. | M5 |
| M10 | Stats & Calendar | Stats service + UI (incl. reading recap), calendar, activity log polish. | M6 |
| M11 | Backup & Recovery | Backup/restore/rotation/validation, auto-backup, merge with conflict preview. | M2, M3, M6 |
| M12 | UX Polish | Shortcuts complete, command palette full, states audit, a11y pass, RTL pass, performance pass. | M5–M11 |
| M13 | Testing & Release | Integration/E2E suites, benchmarks, packaging, Alpha → Beta → Stable. | all |
| FX | Future Scope | Post-Stable, behind designed seams: cloud sync, plugins, AI (opt-in), mobile, more importers/content types. | M13 |

**Dependency spine:** M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8 → M9 → M10 → M11 → M12 → M13.
**Parallel tracks after M3:** (M4 UI shell) ‖ (M5 library) ‖ (M7 provider work once M2+M3 land).

---

## 3. Mission list (everything)

Legend — **Pri:** Core (must ship) · Important (should ship) · Optional (if time, no scope creep).
**Cplx:** S (≤1 day) · M (2–5 days) · L (1–2 weeks). States:
`BACKLOG · READY · IN_PROGRESS · REVIEW · TESTING · DONE · BLOCKED · CANCELLED`.

**Mission log** (most recent first):

| Mission | Status |
|---------|--------|
| MISSION-021 Benchmarks: insert 1k/10k, search 10k/50k/100k rows, bulk import timing | **DONE** (2026-08-11) — `src-tauri/benches/database.rs` (criterion, release) ✓ · fresh in-memory migrated DB per insert sample (results independent of prior samples) ✓ · insert repo path (`media::create` per-row tx) 1k ≈ 349 ms (~0.35 ms/row), 10k ≈ 3.66 s ✓ · insert bulk raw one-tx 1k ≈ 184 ms, 10k ≈ 1.90 s (~2× faster) ✓ · search FTS5 **flat at ~110 µs** across 10k/50k/100k rows (selective ~1% hit + no-match) — index-bound, not data-bound ✓ · bulk import: raw one-tx 10k ≈ 1.90 s vs repo loop ≈ 3.76 s → import pipeline (MISSION-066) must batch in one tx ✓ · DATABASE.md §5.2 ✓ · clippy -D warnings ✓ · 70/70 tests ✓ |
| MISSION-020 DB integration tests: CRUD, FKs, cascade, FTS query, transaction rollback | **DONE** (2026-08-11) — `infrastructure::integration_tests` (cfg(test)) over the fully-migrated schema ✓ · full lifecycle: create media + nodes/tracking/progress/review/collection/activity → delete media → every aggregate cascades, FTS empties, unrelated media + collection + person survive ✓ · manual tx: failed statement → whole tx rolls back (incl. FTS), commit persists ✓ · repo-internal atomicity: failing link rolls back `media::create` AND `media::update` (previous aggregate untouched) ✓ · FTS follows review body edits (index add + clear + delete) ✓ · FK rejection at repo boundary for every aggregate ✓ · 70/70 tests ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-019 Repositories: media, node, tracking, review, collection, asset, activity (sqlx typed) | **DONE** (2026-08-11) — `infrastructure::repositories/{media,node,tracking,review,collection,asset,activity}` ✓ · media: full-aggregate create/get/update (row + links in one tx), delete, `find_by_external_id`, `list` (content_type/pub_status/genre/tag/favorite filters, sort, pagination) + `count`, FTS `search` (unicode61 + trigram, `fts::normalize_query` Arabic fold) ✓ · node: create/reparent/update/delete/children with `content_node` validators (cross-media + cycle rejection) ✓ · tracking: row upsert + `node_progress` upsert/read + `count_nodes_in_state` ✓ · review/collection/asset/activity CRUD ✓ · repos are clock-free, never write FTS tables ✓ · 64/64 tests ✓ · clippy -D warnings ✓ · DATABASE.md §5.1 ✓ |
| MISSION-018 FTS5 `media_fts` + triggers + rebuild; multilingual tokenization (unicode61 + trigram for CJK) | **DONE** (2026-08-11) — `migrations/0007_media_fts.sql` ✓ · `v_media_fts_source` view with Arabic normalization (Alef variants→ا, ى→ي, ة→ه, diacritics/tanween stripped) ✓ · `media_fts` (unicode61, 9 columns) + `media_fts_cjk` (trigram) — **stored content**, not contentless (FTS5 contentless `'delete'` either needs deleted values or silently orphans terms) ✓ · 21 refresh triggers (ins/del/upd on media + 6 feeder tables) + backfill ✓ · `PRAGMA recursive_triggers = ON` in `db::connect` + `test_support::in_memory_pool` so cascades re-fire FTS triggers ✓ · `infrastructure::fts::rebuild` (transactional wipe + repopulate) ✓ · 45/45 tests (schema/triggers, both indexes, CJK trigram substring, Arabic fold, alt-title refresh, cascade refresh, update refresh, rebuild) ✓ · clippy -D warnings ✓ |
| MISSION-017 `review`, `collection`, `collection_member`, `asset`, `activity`, `trash`, `settings` | **DONE** (2026-08-11) — `migrations/0006_user_aggregates.sql` ✓ · media cover/banner asset columns added via ALTER TABLE ADD COLUMN (deferred FK from 0002) ✓ · CHECKs: rating 1..10, favorites/spoiler/smart/restored booleans, asset kind/status, activity kind, trash kind ✓ · `idx_activity_media` ✓ · `provider_setting` deferred to MISSION-063 ✓ · 36/36 tests (asset SET NULL, review/collection/activity cascades, checks, settings roundtrip) ✓ · clippy -D warnings ✓ |
| MISSION-016 `tracking`, `status` (seeded core), `node_progress` + CHECKs | **DONE** (2026-08-11) — `migrations/0005_tracking.sql` ✓ · 7 core statuses seeded (is_system=1) ✓ · CHECKs: core_status, bucket, repeat_count>=0, node state, rating 1..10 ✓ · `idx_tracking_core_status`/`idx_tracking_updated_at` ✓ · 30/30 tests (seeds, checks, FK cascade, custom-status SET NULL) ✓ · clippy -D warnings ✓ |
| MISSION-015 `media_external_id`, `media_relation` + unique constraints | **DONE** (2026-08-11) — `migrations/0004_media_identity.sql` ✓ · PK(provider, ext_id) + UNIQUE(media_id, provider) exact-identity rules ✓ · relation CHECK + CHECK(from_id <> to_id) ✓ · 25/25 tests (uniques, cascades, self/missing/bad-relation rejection) ✓ · clippy -D warnings ✓ |
| MISSION-014 `content_node` tree + indexes; FK validation helpers | **DONE** (2026-08-11) — `migrations/0003_content_node.sql` (kind CHECK, `idx_node_media`/`idx_node_parent`) ✓ · `infrastructure::content_node` validators: parent-belongs-to-media + acyclic (SQLite can't express cross-row tree invariants) ✓ · `AppError::Validation` ✓ · shared test helpers moved to `test_support` ✓ · 21/21 tests (schema/indexes, media-cascade, kind CHECK, cross-media + cycle rejection) ✓ · clippy -D warnings ✓ |
| MISSION-013 `media`, `media_alt_title`, `person`/`media_person`, `genre`, `tag` (+ joins) + seeds | **DONE** (2026-08-11) — `migrations/0002_media.sql` ✓ · enum CHECKs (content_type, pub_status, role, scope) ✓ · seeds: 18 genres + 21 domain tags ✓ · asset FK columns deferred to MISSION-017 (SQLite rejects writes through an FK to a missing parent) ✓ · 16/16 tests (schema+seeds, media/person cascade, FK enforced) ✓ · clippy -D warnings ✓ |
| MISSION-012 Migration runner (`sqlx::migrate!`), versioned `migrations/`, transaction-wrapped | **DONE** (2026-08-11) — `db::migrate` wired into `init` ✓ · `migrations/0001_init.sql` (`app_meta`) ✓ · per-migration transaction verified in sqlx-sqlite 0.8.6 ✓ · `AppError::Migration` ✓ · idempotency + corrupt-db tests (Windows WAL/SHM lock fix) ✓ · clippy -D warnings ✓ |
| MISSION-011 sqlx pool + PRAGMAs + startup integrity check | **DONE** (2026-08-11) — `db::init` (FK/WAL/busy_timeout=5s/synchronous=Normal, max 5 conns) ✓ · `PRAGMA integrity_check` at startup ✓ · pool as managed state (`app.manage`) ✓ · `AppError::Database` ✓ · tests ✓ · clippy -D warnings ✓ |
| MISSION-010 Window shell: titlebar, min sizes, app id, CSP | **REVIEW** — productName/identifier `MyLore`/`com.mylore.app` ✓ · window 1100x760 min 900x600 centered ✓ · strict prod CSP (no unsafe-inline) + dev CSP ✓ · config guard test ✓ · cargo build validates ✓ · 7/7 tests ✓ |
| MISSION-009 Typed IPC boundary + codegen | **REVIEW** — contract in `scripts/ipc-contract.json` ✓ · `npm run codegen` → `src/api/ipc.generated.ts` ✓ · drift-guard (Rust #[command] scan + --check) ✓ · App.tsx uses typed `greet` ✓ · 4/4 tests ✓ · lint/build ✓ |
| MISSION-008 Logging (tracing → rolling file, no secrets) + `AppError` | **REVIEW** — clippy -D warnings ✓ · 6/6 tests ✓ · cargo build ✓ · rolling daily logs + stdout ✓ · AppError→frontend as string ✓ |
| MISSION-007 GitHub Actions CI (win/ubuntu/macos) | **REVIEW** — actionlint ✓ · prettier ✓ · all steps pass locally (fmt/clippy/test/build/lint) |
| MISSION-006 Rust unit-test harness + in-memory sqlite helper | **DONE** (2026-08-11) — `cargo test` 2/2 ✓ · clippy -D warnings ✓ · fmt ✓ |
| MISSION-005 Vitest + Testing Library harness + sample test | **DONE** (2026-08-11) — 2 tests pass · lint ✓ · format ✓ · build ✓ |
| MISSION-004 Rust rustfmt + Clippy + crate layout | **DONE** (2026-08-11) — fmt ✓ · clippy -D warnings ✓ · layered crate layout per PROJECT_MAP |
| MISSION-003 ESLint + Prettier + pre-commit hook | **DONE** (2026-08-11) — eslint ✓ · prettier ✓ · husky+lint-staged ✓ · .gitattributes ✓ |
| MISSION-002 TS strict + path aliases + editor config | **DONE** (2026-08-11) — strict flags ✓ · `@/` alias (tsc+vite) ✓ · .editorconfig ✓ |
| MISSION-001 Scaffold Tauri 2 + React + TS | **DONE** (2026-08-11) — npm build ✓ · cargo build ✓ · app launched & user-tested ✓ |

### M1 · Foundation (MISSION-001 … 010)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-001 | Scaffold Tauri 2 + React + TS (Vite); baseline config (`tauri.conf.json`, capabilities). | — | Core | M |
| MISSION-002 | TypeScript strict config, path aliases, editor config. | 001 | Core | S |
| MISSION-003 | ESLint + Prettier (+ pre-commit hook via lint-staged). | 002 | Core | S |
| MISSION-004 | Rust: rustfmt + Clippy (deny warnings in CI), crate layout. | 001 | Core | S |
| MISSION-005 | Vitest + @testing-library/react harness; sample test. | 002 | Core | S |
| MISSION-006 | Rust unit-test harness (`cargo test`) with in-memory sqlite helper. | 004 | Core | S |
| MISSION-007 | GitHub Actions: build + lint + test (windows/ubuntu/macos-latest). | 003..006 | Core | M |
| MISSION-008 | Logging (tracing → rolling file, no secrets) + `AppError` skeleton. | 001 | Core | M |
| MISSION-009 | Typed IPC boundary: shared TS types for commands/events + codegen. | 002,008 | Core | M |
| MISSION-010 | Window shell: titlebar, min sizes, app identifier, CSP in config. | 001 | Core | S |

### M2 · Database (MISSION-011 … 021)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-011 | sqlx pool + PRAGMAs (FK, WAL, busy_timeout) as managed state; startup integrity check. | 008 | Core | M |
| MISSION-012 | Migration runner (`sqlx::migrate!`), versioned `migrations/`, transaction-wrapped. | 011 | Core | S |
| MISSION-013 | `media`, `media_alt_title`, `person`/`media_person`, `genre`, `tag` tables + seeds. | 012 | Core | M |
| MISSION-014 | `content_node` tree + indexes; FK validation helpers. | 013 | Core | M |
| MISSION-015 | `media_external_id`, `media_relation` + unique constraints. | 014 | Core | M |
| MISSION-016 | `tracking`, `status` (seeded core), `node_progress` + CHECKs. | 013,014 | Core | M |
| MISSION-017 | `review`, `collection`, `collection_member`, `asset`, `activity`, `trash`, `settings`. | 013..016 | Core | M |
| MISSION-018 | FTS5 `media_fts` + triggers + rebuild; multilingual tokenization (unicode61 + trigram for CJK). | 013,015,016 | Core | L |
| MISSION-019 | Repositories: media, node, tracking, review, collection, asset, activity (sqlx typed). | 011..018 | Core | L |
| MISSION-020 | DB integration tests (CRUD, FKs, cascade, FTS query, transaction rollback). | 019 | Core | M |
| MISSION-021 | Benchmarks: insert 1k/10k, search 10k/50k/100k rows, bulk import timing. | 020 | Important | M |

### M3 · Domain Layer (MISSION-022 … 029)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-022 | Domain types: `Media`, `ContentNode`, `Tracking`, `Review`, value objects; invariant guards. | 012 | Core | M |
| MISSION-023 | Progress engine: per-contentType templates, aggregates (pages/chapters/episodes); unit tests. | 022 | Core | M |
| MISSION-024 | Status engine: core statuses, custom statuses, auto-transition rules (reversible). | 022 | Core | M |
| MISSION-025 | Title normalization (case/unicode/diacritic fold, script-aware) + title matching. | 022 | Core | M |
| MISSION-026 | IdentityService: exact (provider, ext_id) + fuzzy scoring + candidate ranking. | 025 | Core | M |
| MISSION-027 | StatsService: pure computations (counts, hours, completion, avg rating, distributions). | 022 | Core | M |
| MISSION-028 | MergeService: merge plan, conflict report, re-parenting, before-image. | 022,026 | Important | L |
| MISSION-029 | Service unit tests: progress math, status, dedup, stats, merge. | 023..028 | Core | M |

### M4 · UI Foundation (MISSION-030 … 037)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-030 | Design tokens (colors/type/spacing/radius/elevation), light+dark themes, `data-theme`. | 010 | Core | M |
| MISSION-031 | Tailwind + design-system primitives (Button, Input, Dialog, Popover, Toast, Skeleton…) on Radix. | 030 | Core | L |
| MISSION-032 | Router + app shell: nav rail, topbar, status bar; empty-state pages. | 031 | Core | M |
| MISSION-033 | i18n: i18next en/ar, ICU, RTL wiring (`dir`, logical props), locale switcher. | 032 | Core | M |
| MISSION-034 | Settings store (tauri-plugin-store) + preferences model; theme/lang persistence. | 033 | Core | S |
| MISSION-035 | TanStack Query client + typed command wrappers (`api.ts`) + query keys. | 009,032 | Core | M |
| MISSION-036 | Command palette skeleton (Ctrl/Cmd+K) + shortcut registry. | 032 | Important | M |
| MISSION-037 | A11y baseline: focus ring, reduced-motion, semantic shell, screen-reader pass. | 031 | Important | M |

### M5 · Library MVP (MISSION-038 … 045)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-038 | Manual Add dialog (fast entry, validation with Zod) → MediaService command. | 019,022,032 | Core | M |
| MISSION-039 | Library query endpoint (filter/sort/group/paginate) + API. | 019 | Core | M |
| MISSION-040 | Library views: Grid / List / Compact list (virtualized, TanStack Virtual). | 035,039 | Core | L |
| MISSION-041 | Filter panel (type, format, status, genre, tag, year, favorite) + sort menu + group-by. | 039,040 | Core | L |
| MISSION-042 | Media detail page: hero, meta tabs, actions (overview/detail/tracking/review shell). | 035,039 | Core | L |
| MISSION-043 | Local search (FTS) in header + results page (local-first). | 018,039 | Core | M |
| MISSION-044 | Trash/restore UI + undo toast for deletes. | 017,038 | Important | M |
| MISSION-045 | Bulk-select mode + action bar (status, tag, list, delete, export later). | 040,044 | Important | M |

### M6 · Tracking (MISSION-046 … 052)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-046 | Node tree endpoint + UI (seasons→episodes, volumes→chapters; expand/collapse). | 014,019,042 | Core | L |
| MISSION-047 | Per-node progress commands (mark read/watched/skipped, range-mark) + optimistic UI. | 023,046 | Core | M |
| MISSION-048 | Status transitions + auto-complete rule; repeat counter. | 024,047 | Core | M |
| MISSION-049 | Quick capture popover (global hotkey) + in-grid quick controls. | 047,036 | Core | M |
| MISSION-050 | Dashboard widgets: Continue Reading/Watching, Recently Added/Completed, Quick Actions. | 047,048,039 | Core | M |
| MISSION-051 | Activity log writes on all tracking actions. | 017,047 | Core | S |
| MISSION-052 | **Novel/web-novel tracking UX** (research §2b): chapter-list read-state rows + "my status" markers; **Normal (autoTrack) vs Manual mode** per media; **DNF-with-progress** (dropped carries % / chapter). | 047,048 | Important | M |

### M7 · Providers (MISSION-053 … 065)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-053 | Provider trait + capabilities + Coordinator (rate limit, retry/backoff, timeout, cancel, error map). | 022 | Core | L |
| MISSION-054 | AniList adapter (search/details/nodes/relations/external ids; LNs via MANGA/NOVEL) + fixtures. | 053 | Core | L |
| MISSION-055 | TMDB adapter (movies+TV, seasons/episodes, images, attribution) + fixtures. | 053 | Core | L |
| MISSION-056 | MangaDex adapter (manga/manhwa/manhua, chapters, covers) + fixtures. | 053 | Core | L |
| MISSION-057 | OpenLibrary adapter (books, ISBN, covers) + fixtures. | 053 | Core | M |
| MISSION-058 | Fallbacks: Jikan (anime), Google Books (books). | 054,057 | Optional | M |
| MISSION-059 | External search UI (grouped by provider, "in library"/duplicate flags). | 053..057,026 | Core | L |
| MISSION-060 | Import-from-provider flow (search → details → identity check → add). | 059,026 | Core | M |
| MISSION-061 | Enrich metadata (refresh provider fields only; never touch user data) + diff report. | 054..057,042 | Core | M |
| MISSION-062 | Image pipeline: download/cache covers, broken-url handling, cache policy. | 017,053 | Core | M |
| MISSION-063 | Provider settings UI: enable/disable, API keys (keyring), test connection. | 053 | Core | M |
| MISSION-064 | Hardcover adapter (books/LN, free GraphQL) — optional third book provider. | 057 | Optional | M |
| MISSION-065 | Bangumi adapter (CN ACGN incl. LN/WN, open API, ~1 rps) — optional. | 053 | Optional | M |

### M8 · Import / Export (MISSION-066 … 072)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-066 | Import pipeline core: parser→validator→normalizer→dedup→preview→txn→report. | 026,019 | Core | L |
| MISSION-067 | JSON import (app format) + CSV import (mapping UI). | 066 | Core | M |
| MISSION-068 | Bulk-import preview UI (per-item outcome) + confirm/cancel. | 066,040 | Core | M |
| MISSION-069 | Background TaskManager (progress, cancel, typed results) wired to import. | 066 | Core | M |
| MISSION-070 | Export JSON / CSV / Markdown (streaming, dialog pick). | 019 | Core | M |
| MISSION-071 | Provider imports: AniList user export, MAL (Jikan), **Goodreads CSV**, **StoryGraph CSV**, Trakt (optional). | 066,054,058 | Optional | M |
| MISSION-072 | Import/export integration tests + fixture data. | 067..070 | Core | M |

> Note: novels/web novels/light novels have no clean open provider (NovelUpdates: no API, ToS
> forbids scraping; AniList indexes LNs only). Use OpenLibrary/Google Books (+ optional Hardcover/
> Bangumi) for metadata; adopt NovelUpdates' genre/tag taxonomy as conventions. Book imports come
> from Goodreads/StoryGraph CSV (user-owned data). `API_PROVIDERS.md` §12–15.

### M9 · Reviews & Collections (MISSION-073 … 078)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-073 | Review/notes UI (rating, review, short review, spoiler, personal tags) + commands. | 017,042 | Core | M |
| MISSION-074 | Favorites flag in grid/list + filter. | 039 | Core | S |
| MISSION-075 | Collections CRUD + drag/drop membership + bulk add. | 017,040 | Core | M |
| MISSION-076 | Smart collections: save filter as collection; query builder (basic). | 039,075 | Important | M |
| MISSION-077 | Bulk operations on filtered selection (status/tag/list/delete) with summary. | 045 | Important | M |
| MISSION-078 | **Mood / pace / content-warning badges** on detail page (StoryGraph); content warnings as acknowledged metadata. | 073,042 | Optional | M |

### M10 · Stats & Calendar (MISSION-079 … 082)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-079 | StatsService endpoints + Stats page (cards + small charts, tabular numbers). | 027,019 | Important | M |
| MISSION-080 | Calendar: local air/release dates + activity; month grid + list. | 051,039 | Optional | M |
| MISSION-081 | Year-in-review style recap. | 079 | Optional | M |
| MISSION-082 | **Reading recap stats**: pages/chapters per month, mood/pace/format trends (StoryGraph) — local-data-only. | 079 | Optional | M |

### M11 · Backup & Recovery (MISSION-083 … 088)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-083 | BackupService: `VACUUM INTO` snapshot + assets + meta → `.mylore`; validate. | 011 | Core | M |
| MISSION-084 | Restore: quarantine current, swap, verify, rollback-safe. | 083 | Core | M |
| MISSION-085 | Automatic backup schedule + rotation (N + monthly) in preferences. | 083 | Core | M |
| MISSION-086 | Pre-migration auto-backup hook. | 083,012 | Core | S |
| MISSION-087 | Backups UI (list, restore, validate, delete) + recovery from corrupt DB (prompt restore). | 083,084 | Core | M |
| MISSION-088 | Merge UI with conflict preview + restore-from-trash for merges. | 028 | Important | L |

### M12 · UX Polish (MISSION-089 … 094)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-089 | Complete shortcut map + command palette (add, mark complete, status, navigate, settings). | 036 | Important | M |
| MISSION-090 | States audit: empty/loading/skeleton/error/retry on every data surface. | 040..050 | Core | M |
| MISSION-091 | RTL pass: mirror nav, flip icons, mixed-direction titles, test AR+EN. | 033 | Core | M |
| MISSION-092 | Accessibility pass (WCAG AA): focus, labels, dialogs, contrast, reduced-motion. | 037 | Important | M |
| MISSION-093 | Performance pass: virtual lists, debounced search, image cache, startup timing. | 021,040 | Important | M |
| MISSION-094 | Micro-interactions & density tiers (comfortable/compact). | 031 | Optional | M |

### M13 · Testing & Release (MISSION-095 … 099)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-095 | Integration tests: DB, providers (recorded fixtures), import/export, backup/restore. | all | Core | L |
| MISSION-096 | E2E (Playwright + tauri-driver): add media, search, track progress, import, backup, restore. | 095 | Core | L |
| MISSION-097 | Provider mock harness + fixtures committed (offline CI). | 053..057 | Core | M |
| MISSION-098 | Release pipeline: build installers (Win/mac/Linux), signing (Win/mac), versioning, changelog. | 007 | Core | L |
| MISSION-099 | Alpha → Beta → Stable gates + `MILESTONE-REPORT.md` per milestone. | all | Core | M |

### FX · Future Scope (MISSION-100+)

Post-Stable, behind designed seams. Nothing here blocks M1–M13 (ADR-013 scope discipline).

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-100 | Cloud sync: aggregate-level last-write-wins + conflict resolution (`updatedAt` already designed). | 099 | Optional | L |
| MISSION-101 | Trakt import + scrobble integration. | 099 | Optional | M |
| MISSION-102 | SIMKL import source. | 099 | Optional | M |
| MISSION-103 | Plugins: provider-adapter plugin seam (first plugin surface). | 099 | Optional | L |
| MISSION-104 | AI features (optional, local, disable-able): auto-tag suggestions, summaries. | 099 | Optional | L |
| MISSION-105 | Mobile companion (read-only or lightweight tracking). | 099 | Optional | L |
| MISSION-106 | WN/LN chapter-release notifications + release-feed calendar (NovelUpdates-style Normal mode). | 099 | Optional | M |
| MISSION-107 | Buddy reads with progress-gated spoiler protection (StoryGraph-style; multi-user). | 099 | Optional | L |
| MISSION-108 | New content types: games, podcasts, music (data-only additions: contentType + progress template). | 099 | Optional | M |
| MISSION-109 | NoviList import source (WN/LN tracker with API docs; young, watch). | 099 | Optional | M |
| MISSION-110 | ISBNDB paid enrichment fallback (free tier: 100 req/mo). | 099 | Optional | S |
| MISSION-111 | SQLCipher encrypted database (opt-in). | 099 | Optional | L |
| MISSION-112 | Advanced visualizations/chart library for stats. | 099 | Optional | M |

---

## 4. Execution workflow (per mission)

1. **Pick up** a mission in the current milestone (only READY/BACKLOG missions; never skip deps).
2. Create its checklist in the epic folder (from the template in `DEVELOPMENT_PLAN.md §5`).
3. **Implement → test → review → fix → update docs → set status** (spec §90, §91).
4. Run quality gates for the milestone (TypeScript PASS · Rust build PASS · Lint PASS · Tests
   PASS · Migrations PASS · no console/TS/lint errors · no broken deps).
5. Mark mission DONE only when its acceptance criteria pass; update this roadmap's status.

## 5. Definition of Done & Quality Gates

Implementation ✓ · Type safety ✓ · Tests ✓ · Error handling ✓ · UI validation ✓ · Docs updated ✓ ·
A11y where needed ✓ · Perf where needed ✓ · No console/TS/lint errors ✓ · No broken deps ✓.

Release gates (M13): TS PASS · Rust build PASS · Lint PASS · Tests PASS · Migrations PASS ·
Import/Export PASS · Backup/Restore PASS · Critical UX flows PASS · Security review PASS.

## 6. MVP scope (what ships as "Stable 1.0")

M1–M6 + M7 core providers (AniList, TMDB, MangaDex, OpenLibrary) + M8 (JSON/CSV import-export,
Goodreads/StoryGraph CSV) + M9 basics + M11 backups. Concretely: **Media CRUD · Library ·
Search(local) · Tracking (incl. novel chapter UX) · Provider import/search · Reviews · Tags ·
Collections · Backup/Restore** — fully offline, en/ar RTL.

## 7. Doc roles

| Doc | Role |
|-----|------|
| **ROADMAP.md** (this) | Master: milestones + complete mission list + status. |
| `DEVELOPMENT_PLAN.md` | Reference: per-task detail (files, tests, AC) + traceability. |
| `PROJECT_REQUIREMENTS.md` | What we build (requirements, priorities). |
| `PHASE0_REPORT.md` | Executive summary of Phase 0. |
| `RESEARCH.md` · `API_PROVIDERS.md` · `UX_RESEARCH.md` | Why we build it that way. |
| `ARCHITECTURE.md` · `DOMAIN_MODEL.md` · `DATABASE.md` · `DECISIONS.md` · `DESIGN_SYSTEM.md` | How. |
