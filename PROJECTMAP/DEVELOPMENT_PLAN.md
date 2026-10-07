# MyLore — Development Plan

> Phase 0 · Incremental Implementation Protocol · August 2026
> Rule (spec §61): never build everything at once. Milestones → Epics → Tasks → Subtasks.
> Task states: BACKLOG · READY · IN_PROGRESS · BLOCKED · REVIEW · TESTING · DONE · CANCELLED
> **Master roadmap & mission list: `ROADMAP.md`.** This file is the detailed task reference
> (files, tests, acceptance criteria, traceability). MISSION-NNN ≈ TASK-NNN for the original set.

---

## 1. Milestones

| MS | Name | Exit criterion |
|----|------|----------------|
| M0 | Research & Design | **This document set** (Phase 0). |
| M1 | Foundation | Tauri 2 app builds on Win/macOS/Linux; TS strict + lint + fmt + CI green; typed IPC skeleton; empty window shell. |
| M2 | Database | SQLite via sqlx: migrations, full schema, repositories, FTS5 index, integrity pragmas; tests green. |
| M3 | Domain Layer | Domain entities/services in Rust with unit tests (tracking math, dedup, status transitions, stats). |
| M4 | UI Foundation | Design tokens, themes, router shell, layout + nav rail, i18n (en/ar, RTL), command palette skeleton. |
| M5 | Library MVP | Manual add, media CRUD, library grid/list/compact, filters + sort, media detail page, trash/undo basics. |
| M6 | Tracking | Node trees, per-node progress, quick capture, status transitions, dashboard "continue" widgets. |
| M7 | Providers | Coordinator + AniList, TMDB, MangaDex, OpenLibrary (+ Jikan/Google fallback); external search; enrich; identity/dedup. |
| M8 | Import/Export | Import pipeline + preview + reports; JSON/CSV/Markdown export; provider import (AniList/MAL via Jikan) optional. |
| M9 | Reviews & Collections | Reviews/notes/tags, favorites, collections + smart lists, bulk operations. |
| M10 | Stats & Calendar | Stats service + UI, calendar, activity log polish. |
| M11 | Backup & Recovery | Backup/restore/rotation/validation, auto-backup, merge with conflict preview. |
| M12 | UX Polish | Shortcuts complete, command palette full, states audit, a11y pass, RTL pass, performance pass. |
| M13 | Testing & Release | Integration/E2E suites, benchmarks, packaging, Alpha → Beta → Stable. |

Dependency spine: M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8 → M9 → M10 → M11 → M12 → M13.
Parallel tracks allowed after M3: (M4 UI shell) ‖ (M5 library) ‖ (M7 provider work once M2+M3 land).

## 2. MVP scope (spec §60)

M1–M6 + minimal M7 (search + enrich from one anime and one book provider) + M8 (JSON/CSV import +
export) + basic review/tags + M11 backups. Concretely: **Media CRUD · Library · Search(local) ·
Tracking · Basic Provider Import · Reviews · Tags · Backup/Restore**. Everything else follows in
the same milestone order without scope creep.

## 3. Epics & Tasks

Format per task: `TASK-NNN — Title · Deps: … · Pri: Core|Important|Optional · Files · Test(s) · AC`.

### EPIC-001 Project Foundation (M1)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-001 | Scaffold Tauri 2 + React + TS (Vite); baseline config (tauri.conf.json, capabilities). | — | Core |
| TASK-002 | TypeScript strict config, path aliases, editor config. | TASK-001 | Core |
| TASK-003 | ESLint + Prettier (+ plugin per commit hook via lint-staged). | TASK-002 | Core |
| TASK-004 | Rust: rustfmt + Clippy (deny warnings in CI), crate layout. | TASK-001 | Core |
| TASK-005 | Vitest + @testing-library/react harness on frontend; sample test. | TASK-002 | Core |
| TASK-006 | Rust unit-test harness (`cargo test`) with sqlite in-memory helper. | TASK-004 | Core |
| TASK-007 | GitHub Actions: build+lint+test (windows/ubuntu/macos-latest). | TASK-003..006 | Core |
| TASK-008 | Logging (tracing→rolling file, no secrets) + error type skeleton `AppError`. | TASK-001 | Core |
| TASK-009 | Typed IPC boundary: shared TS types for commands/events + codegen script. | TASK-002,008 | Core |
| TASK-010 | Window shell: titlebar, min sizes, app identifier, CSP in config. | TASK-001 | Core |

### EPIC-002 Database (M2)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-011 | sqlx pool + PRAGMAs (FK, WAL, busy_timeout) as managed state; startup integrity check. | TASK-008 | Core |
| TASK-012 | Migration runner (`sqlx::migrate!`), versioned `migrations/`, transaction-wrapped. | TASK-011 | Core |
| TASK-013 | `media`, `media_alt_title`, `person`/`media_person`, `genre`, `tag` tables + seeds. | TASK-012 | Core |
| TASK-014 | `content_node` tree + indexes; FK validation helpers. | TASK-013 | Core |
| TASK-015 | `media_external_id`, `media_relation` + unique constraints. | TASK-014 | Core |
| TASK-016 | `tracking`, `status` (seeded core), `node_progress` + CHECKs. | TASK-013,014 | Core |
| TASK-017 | `review`, `collection`, `collection_member`, `asset`, `activity`, `trash`, `settings`. | TASK-013..016 | Core |
| TASK-018 | FTS5 `media_fts` + triggers + rebuild command; multilingual tokenization. | TASK-013,015,016 | Core |
| TASK-019 | Repositories: media, node, tracking, review, collection, asset, activity (sqlx typed). | TASK-011..018 | Core |
| TASK-020 | DB integration tests (CRUD, FKs, cascade, FTS query, transaction rollback). | TASK-019 | Core |
| TASK-021 | Benchmarks: insert 1k/10k, search 10k/50k/100k rows, bulk import timing. | TASK-020 | Important |

### EPIC-003 Domain Layer (M3)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-022 | Domain types: `Media`, `ContentNode`, `Tracking`, `Review`, value objects; invariant guards. | TASK-012 | Core |
| TASK-023 | Progress engine: per-contentType templates, aggregates (pages/chapters/episodes), unit tests. | TASK-022 | Core |
| TASK-024 | Status engine: core statuses, custom statuses, auto-transition rules (reversible). | TASK-022 | Core |
| TASK-025 | Title normalization (case/unicode/diacritic fold, script-aware) + title matching. | TASK-022 | Core |
| TASK-026 | IdentityService: exact (provider,ext_id) + fuzzy scoring + candidate ranking. | TASK-025 | Core |
| TASK-027 | StatsService: pure computations (counts, hours, completion, avg rating, distributions). | TASK-022 | Core |
| TASK-028 | MergeService: merge plan, conflict report, re-parenting, before-image. | TASK-022,026 | Important |
| TASK-029 | Service unit tests (spec §54): progress math, status, dedup, stats, merge. | TASK-023..028 | Core |

### EPIC-004 UI Foundation (M4)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-030 | Design tokens (colors/type/spacing/radius/elevation), light+dark themes, `data-theme`. | TASK-010 | Core |
| TASK-031 | Tailwind + design-system primitives (Button, Input, Dialog, Popover, Toast, Skeleton…) on Radix. | TASK-030 | Core |
| TASK-032 | Router + app shell: nav rail, topbar, status bar; empty-state pages. | TASK-031 | Core |
| TASK-033 | i18n: i18next en/ar, ICU, RTL wiring (`dir`, logical props), locale switcher. | TASK-032 | Core |
| TASK-034 | Settings store (tauri-plugin-store) + preferences model; theme/lang persistence. | TASK-033 | Core |
| TASK-035 | TanStack Query client + typed command wrappers (`api.ts`) + query keys. | TASK-009,032 | Core |
| TASK-036 | Command palette skeleton (Ctrl/Cmd+K) + shortcut registry. | TASK-032 | Important |
| TASK-037 | A11y baseline: focus ring, reduced-motion, semantic shell, screen-reader pass. | TASK-031 | Important |

### EPIC-005 Library (M5)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-038 | Manual Add dialog (fast entry, validation with Zod) → MediaService command. | TASK-019,022,032 | Core |
| TASK-039 | Library query endpoint (filter/sort/group/paginate) + API. | TASK-019 | Core |
| TASK-040 | Library views: Grid / List / Compact list (virtualized, TanStack Virtual). | TASK-035,039 | Core |
| TASK-041 | Filter panel (type, format, status, genre, tag, year, favorite) + sort menu + group-by. | TASK-039,040 | Core |
| TASK-042 | Media detail page: hero, meta tabs, actions (overview/detail/tracking/review shell). | TASK-035,039 | Core |
| TASK-043 | Local search (FTS) in header + results page (local-first). | TASK-018,039 | Core |
| TASK-044 | Trash/restore UI + undo toast for deletes. | TASK-017,038 | Important |
| TASK-045 | Bulk-select mode + action bar (status, tag, list, delete, export later). | TASK-040,044 | Important |

### EPIC-006 Tracking (M6)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-046 | Node tree endpoint + UI (seasons→episodes, volumes→chapters; expand/collapse). | TASK-014,019,042 | Core |
| TASK-047 | Per-node progress commands (mark read/watched/skipped, range-mark) + optimistic UI. | TASK-023,046 | Core |
| TASK-048 | Status transitions + auto-complete rule; repeat counter. | TASK-024,047 | Core |
| TASK-049 | Quick capture popover (global hotkey) + in-grid quick controls. | TASK-047,036 | Core |
| TASK-050 | Dashboard widgets: Continue Reading/Watching, Recently Added/Completed, Quick Actions. | TASK-047,048,039 | Core |
| TASK-051 | Activity log writes on all tracking actions. | TASK-017,047 | Core |

### EPIC-007 Providers (M7)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-052 | Provider trait + capabilities + Coordinator (rate limit, retry/backoff, timeout, cancel, error map). | TASK-022 | Core |
| TASK-053 | AniList adapter (search/details/nodes/relations/external ids) + fixtures. | TASK-052 | Core |
| TASK-054 | TMDB adapter (movies+TV, seasons/episodes, images, attribution) + fixtures. | TASK-052 | Core |
| TASK-055 | MangaDex adapter (manga/manhwa/manhua, chapters, covers) + fixtures. | TASK-052 | Core |
| TASK-056 | OpenLibrary adapter (books, ISBN, covers) + fixtures. | TASK-052 | Core |
| TASK-057 | Fallbacks: Jikan (anime), Google Books (books). | TASK-053,056 | Optional |
| TASK-058 | External search UI (grouped by provider, "in library"/duplicate flags). | TASK-052..056,026 | Core |
| TASK-059 | Import-from-provider flow (search → details → identity check → add). | TASK-058,026 | Core |
| TASK-060 | Enrich metadata (refresh provider fields only; never touch user data) + diff report. | TASK-053..056,042 | Core |
| TASK-061 | Image pipeline: download/cache covers, broken-url handling, cache policy. | TASK-017,052 | Core |
| TASK-062 | Provider settings UI: enable/disable, API keys (keyring), test connection. | TASK-052 | Core |

### EPIC-008 Import / Export (M8)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-063 | Import pipeline core: parser→validator→normalizer→dedup→preview→txn→report. | TASK-026,019 | Core |
| TASK-064 | JSON import (app format) + CSV import (mapping UI). | TASK-063 | Core |
| TASK-065 | Bulk-import preview UI (per-item outcome) + confirm/cancel. | TASK-063,040 | Core |
| TASK-066 | Background TaskManager (progress, cancel, typed results) wired to import. | TASK-063 | Core |
| TASK-067 | Export JSON / CSV / Markdown (streaming, dialog pick). | TASK-019 | Core |
| TASK-068 | Provider imports: AniList user export, MAL (Jikan), Goodreads CSV, StoryGraph CSV, Trakt (optional). | TASK-063,053,057 | Optional |
| TASK-069 | Import/export integration tests + fixture data. | TASK-064..067 | Core |

> Note: novels/web novels/light novels have no clean *open* provider (NovelUpdates has no API —
> the LNReader plugin, maintained by an NU moderator, publishes authoritative HTML-scrape selectors
> we follow at a modest rate in MISSION-065; AniList indexes LNs only). Use NovelUpdates +
> OpenLibrary/Google Books (+ optional Hardcover/Bangumi adapters later) for metadata; adopt
> NovelUpdates' genre/tag taxonomy as conventions.
> Book imports come from Goodreads/StoryGraph CSV (user-owned data). `API_PROVIDERS.md` §12–17.

### EPIC-009 Reviews & Collections (M9)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-070 | Review/notes UI (rating, review, short review, spoiler, personal tags) + commands. | TASK-017,042 | Core |
| TASK-071 | Favorites flag in grid/list + filter. | TASK-039 | Core |
| TASK-072 | Collections CRUD + drag/drop membership + bulk add. | TASK-017,040 | Core |
| TASK-073 | Smart collections: save filter as collection; query builder (basic). | TASK-039,072 | Important |
| TASK-074 | Bulk operations on filtered selection (status/tag/list/delete) with summary. | TASK-045 | Important |

### EPIC-010 Statistics & Calendar (M10)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-075 | StatsService endpoints + Stats page (cards + small charts, tabular numbers). | TASK-027,019 | Important |
| TASK-076 | Calendar: local air/release dates + activity; month grid + list. | TASK-051,039 | Optional |
| TASK-077 | Year-in-review style recap (optional). | TASK-075 | Optional |

### EPIC-011 Backup & Recovery (M11)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-078 | BackupService: `VACUUM INTO` snapshot + assets + meta → `.mylore`; validate. | TASK-011 | Core |
| TASK-079 | Restore: quarantine current, swap, verify, rollback-safe. | TASK-078 | Core |
| TASK-080 | Automatic backup schedule + rotation (N + monthly) in preferences. | TASK-078 | Core |
| TASK-081 | Pre-migration auto-backup hook. | TASK-078,012 | Core |
| TASK-082 | Backups UI (list, restore, validate, delete) + recovery from corrupt DB (prompt restore). | TASK-078,079 | Core |
| TASK-083 | Merge UI with conflict preview + restore-from-trash for merges. | TASK-028 | Important |

### EPIC-012 UX Polish (M12)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-084 | Complete shortcut map + command palette (add, mark complete, status, navigate, settings). | TASK-036 | Important |
| TASK-085 | States audit: empty/loading/skeleton/error/retry on every data surface. | TASK-040..050 | Core |
| TASK-086 | RTL pass: mirror nav, flip icons, mixed-direction titles, test AR+EN. | TASK-033 | Core |
| TASK-087 | Accessibility pass (WCAG AA): focus, labels, dialogs, contrast, reduced-motion. | TASK-037 | Important |
| TASK-088 | Performance pass: virtual lists, debounced search, image cache, startup timing. | TASK-021,040 | Important |
| TASK-089 | Micro-interactions & density tiers (comfortable/compact). | TASK-031 | Optional |

### EPIC-013 Testing & Release (M13)

| Task | Description | Deps | Pri |
|------|-------------|------|-----|
| TASK-090 | Integration tests: DB, providers (recorded fixtures), import/export, backup/restore. | all | Core |
| TASK-091 | E2E (Playwright + tauri-driver): add media, search, track progress, import, backup, restore. | TASK-090 | Core |
| TASK-092 | Provider mock harness + fixtures committed (offline CI). | TASK-052..056 | Core |
| TASK-093 | Release pipeline: build installers (Win/mac/Linux), signing (Win/mac), versioning, changelog. | TASK-007 | Core |
| TASK-094 | Alpha → Beta → Stable gates + `MILESTONE-REPORT.md` per milestone. | all | Core |

## 4. Dependency graph (high-level)

```
TASK-001 → 002 → 003 → 007
   └─────→ 004 → 006 ──────┘
   └─────→ 008 → 009 → 035
   └─────→ 010 → 030 → 031 → 032 → 033 → 034
                                     └→ 036 · 037
TASK-008 → 011 → 012 → 013 → 014 → 015 → 016 → 017 → 018 → 019 → 020 → 021
                        └→ 022 → 023 → 024 → 025 → 026 → 027 → 028 → 029
EPIC-005 (038..045)  ← EPIC-002 + EPIC-003 + EPIC-004
EPIC-006 (046..051)  ← EPIC-005
EPIC-007 (052..062)  ← EPIC-003 (+ EPIC-005 for search UI)
EPIC-008 (063..069)  ← EPIC-003 + EPIC-007(059)
EPIC-009 (070..074)  ← EPIC-005
EPIC-010 (075..077)  ← EPIC-006
EPIC-011 (078..083)  ← EPIC-002 + EPIC-006 + EPIC-003
EPIC-012 (084..089)  ← EPIC-005..011
EPIC-013 (090..094)  ← everything
```

- **Critical path:** 001→…→020→022→039→040→047→058→059→063→…→093.
- **Parallelizable:** EPIC-004 (UI shell) ‖ EPIC-005 (library, once M2+M3 land); EPIC-007 adapter
  tasks (053–056) are mutually independent; EPIC-010 ‖ EPIC-011.

## 5. Task template (every task records)

`ID · Title · Goal · Dependencies · Files Affected · Implementation Notes · Acceptance Criteria ·
Tests Required · Risk · Priority · Complexity (S/M/L) · Status` (spec §63). Tasks above carry the
key fields; when picked up, fill the remaining ones into a checklist in the epic folder.

## 6. Definition of Done (spec §66)

Implementation ✓ · Type safety ✓ · Tests ✓ · Error handling ✓ · UI validation ✓ · Docs updated ✓ ·
A11y where needed ✓ · Perf where needed ✓ · No console/TS/lint errors ✓ · No broken deps ✓

## 7. Quality gates (spec §85, before any release)

TypeScript PASS · Rust build PASS · Lint PASS · Tests PASS · Migrations PASS · Import/Export PASS ·
Backup/Restore PASS · Critical UX flows PASS · Security review PASS.

## 8. Requirements traceability (spec §88)

| Requirement | Feature | Tasks | Tests |
|---|---|---|---|
| REQ-MEDIA-001/002/003 | Media CRUD + manual add | 013,019,038,039,042 | T-UNIT-013, E2E add-media |
| REQ-MEDIA-005 | External IDs/dedup | 015,026,058,059 | T-UNIT-026 |
| REQ-TRACK-001..005 | Tracking + quick capture | 014,016,023,046..049 | T-UNIT-023, E2E progress |
| REQ-REVIEW-001/002 | Review/notes/tags | 017,070 | T-UNIT-070 |
| REQ-SEARCH-001..003 | Local search | 018,025,043 | T-DB-021, E2E search |
| REQ-IMPORT-001..003 | Import pipeline | 063..066 | T-IT-069, E2E import |
| REQ-EXPORT-001 | Export | 067 | T-IT-069 |
| REQ-BACKUP-001/002 | Backup/restore | 078..082 | T-IT-090, E2E backup |
| REQ-STAT-001 | Stats | 027,075 | T-UNIT-027 |
| REQ-DASH-001 | Dashboard | 050 | E2E dashboard |
| REQ-COLL-001/002 | Collections + bulk | 072..074 | T-IT-074 |
| REQ-UX-003 | i18n/RTL | 033,086 | E2E RTL |
| REQ-PROV-001 | Providers/keys | 052,062 | T-IT-090 |

## 9. Task status flow

`BACKLOG → READY → IN_PROGRESS → REVIEW → TESTING → DONE` (or `BLOCKED` / `CANCELLED`).
After each task: implement → test → review → fix → update docs → update status (spec §90, §91).




