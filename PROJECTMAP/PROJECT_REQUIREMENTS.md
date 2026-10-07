# MyLore — Project Requirements

> Local-First Media Tracker · Personal Local Media Management Platform
> Phase 0 · Discovery & Requirements Analysis · August 2026

---

## 1. Product Vision

MyLore is a **privacy-first, offline-first desktop application** for personally tracking and
managing everything a user consumes: novels, web novels, books, manga/manhwa/manhua, anime,
TV series, and movies — with room for future media types. It is a **local management platform**,
not a thin tracking app: the local database is the *source of truth* for user data; external
services are merely *metadata providers*.

Built with **TypeScript + Tauri 2** (Rust backend, SQLite database), it must be a pleasure to
use daily for years: fast, calm, keyboard-driven, bulk-capable, RTL-ready, fully offline, and
safe from data loss.

### Product Principles

| # | Principle | Meaning |
|---|-----------|---------|
| P1 | Local-first & offline-first | The app works completely without internet. Network is only used for metadata import, external search, and image download. |
| P2 | Privacy by design | No telemetry by default. No collection of reading/watching history on any server. |
| P3 | Data ownership | The local SQLite database is the single source of truth for user data. Metadata refresh never destroys user progress, notes, or ratings. |
| P4 | Provider independence | Domain models are not coupled to any external API. New providers can be added later without redesign. |
| P5 | Modular & maintainable | Clear layering (Domain / Application / Infrastructure / Presentation). No god objects. |
| P6 | Incremental delivery | Milestones → Epics → Features → Tasks. Nothing is built all at once. |
| P7 | Data integrity | Foreign keys, constraints, transactions, validated migrations, backup/restore as a core feature. |
| P8 | Excellent daily workflow | Fast progress capture, keyboard shortcuts, context menus, bulk operations, no unnecessary confirm dialogs. |
| P9 | Scalable to real libraries | Designed for 10,000+ media items and 100k+ episodes/chapters without freezing. |
| P10 | Internationalization | English + Arabic from day one, real RTL, tokenization that works for CJK titles. |

---

## 2. Users & Usage Scenarios

- **Primary user:** a single personal user on their own desktop machine (Windows / macOS / Linux).
- **Secondary:** a small household (multi-user on one device is Future scope).
- **Workflow A — Daily tracking:** user finishes reading a chapter / watching episodes and records
  progress in a few key presses (shortcut → quick capture, in-grid controls).
- **Workflow B — Discover & add:** user searches a title (local first, then providers), picks a
  result, imports metadata, optionally enriches later.
- **Workflow C — Library management:** browse, filter, sort, group, bulk-edit status/tags/lists.
- **Workflow D — Reflection:** write reviews/notes/ratings, read statistics, view calendar.
- **Workflow E — Safety:** create backups, restore, import from other trackers, export data.

---

## 3. Functional Requirements

Every requirement carries an ID used for traceability into `DEVELOPMENT_PLAN.md`
(Requirement → Feature → Tasks → Tests).

### 3.1 Media & Metadata

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-MEDIA-001 | Support multiple content types: book, novel, web novel, manga, manhwa, manhua, anime, TV series, movie, plus extensible types. | Core |
| REQ-MEDIA-002 | Unified metadata model: titles (main/alternative/original), description, cover/banner, authors, artists, directors, studios, publishers, genres, tags, status, dates, language, country, content type. | Core |
| REQ-MEDIA-003 | Manual add of media without any API (fast entry). | Core |
| REQ-MEDIA-004 | Enrich metadata later from a provider without touching user data. | Core |
| REQ-MEDIA-005 | External IDs per provider with cross-provider deduplication. | Core |
| REQ-MEDIA-006 | Merge two records with preview, conflict resolution, and recovery. | Important |
| REQ-MEDIA-007 | Soft delete / trash / restore for destructive operations. | Important |

### 3.2 Tracking & Progress

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-TRACK-001 | Progress model per type: pages (books), chapters (novels, manga*), volumes, episodes (anime/TV), per-season-per-episode (TV), single state (movies). | Core |
| REQ-TRACK-002 | Hierarchical content: Series→Season→Episode; Manga/Novel→Volume→Chapter; flat media allowed. | Core |
| REQ-TRACK-003 | Tracking states: planned, reading/watching, completed, on-hold, dropped, re-reading/re-watching; system core statuses + user-defined custom statuses. | Core |
| REQ-TRACK-004 | Per-node (episode/chapter) state, completed date, notes, optional rating. Aggregate progress is derived, never duplicated. | Core |
| REQ-TRACK-005 | Fast progress capture without opening windows (quick capture, keyboard, in-grid controls). | Core |
| REQ-TRACK-006 | Re-reading / re-watching support (repeat counter). | Important |

### 3.3 Reviews, Notes, Collections

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-REVIEW-001 | Local rating, review, short review, notes, favorite, personal tags, spoiler flag, created/updated dates. | Core |
| REQ-REVIEW-002 | Clear separation of external rating/review vs user rating/review/note. | Core |
| REQ-COLL-001 | Custom lists, favorites, collections, and structured smart lists (filter-based; query builder later). | Core |
| REQ-COLL-002 | Bulk operations on collections: add/remove, change status, add tag, delete, export. | Important |

### 3.4 Search & Discovery

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-SEARCH-001 | Local full-text search across title, alt titles, author, artist, genre, tag, notes, review, chapter/episode, external id. | Core |
| REQ-SEARCH-002 | Separate local vs external (provider) search; combined results with grouping and dedup. | Core |
| REQ-SEARCH-003 | Search that works for Arabic, Japanese, Korean, and Chinese (FTS5 tokenization strategy, section `DATABASE.md`). | Core |
| REQ-DISCOVER-001 | Provider-driven discover surface (browse provider catalogs, seasonal charts for anime). | Optional |

### 3.5 Import / Export / Backup

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-IMPORT-001 | Import paths: manual add, provider import, bulk import, JSON, CSV, backup file. | Core |
| REQ-IMPORT-002 | Import pipeline: parser → validator → normalizer → deduplicator → preview → transaction → result report. | Core |
| REQ-IMPORT-003 | Preview & explicit confirmation before destructive bulk imports. | Core |
| REQ-EXPORT-001 | Export: JSON, CSV, human-readable Markdown, SQLite backup. Provider-specific exports later. | Core |
| REQ-BACKUP-001 | Manual backup, restore, automatic backup, rotation, validation, backup import/export. | Core |
| REQ-BACKUP-002 | No data loss from migration failure, crash, corrupted DB, or bad import. | Core |

### 3.6 Dashboard, Stats, Calendar

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-DASH-001 | Customizable dashboard: continue reading/watching, recently added/completed, statistics, quick actions, favorites; not crowded. | Important |
| REQ-STAT-001 | Accurate statistics: counts per type, episodes/chapters/pages/hours, completion rate, average rating, monthly/yearly activity, genre distribution. | Important |
| REQ-CAL-001 | Local calendar: upcoming releases/episodes, reading & watching activity, completed items. Does not depend on a provider. | Optional |
| REQ-NOTIF-001 | Optional notifications for new episodes/chapters/releases/reminders; fully disable-able. | Optional |

### 3.7 System & UX

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-UX-001 | Keyboard shortcuts, context menus, command palette (Ctrl/Cmd+K). | Important |
| REQ-UX-002 | Theme: dark + light, from day one. | Core |
| REQ-UX-003 | i18n: English + Arabic with full RTL from day one; LTR/RTL tested with long and CJK titles. | Core |
| REQ-UX-004 | Accessibility: keyboard navigation, focus management, contrast, reduced motion, semantic structure, accessible dialogs/forms. | Important |
| REQ-UX-005 | Empty / loading / error states everywhere data is shown. | Core |
| REQ-UX-006 | Preferences: theme, language, default view/sort/status, date/time formats, start-of-week, notifications, provider prefs, image cache, backup settings. | Core |
| REQ-PROV-001 | Provider settings with capability display, API key management (secure storage, test connection, remove). | Core |

---

## 4. Non-Functional Requirements

### 4.1 Performance (NFR-PERF)
- Startup: window visible in ~1s on typical hardware; DB init non-blocking.
- Library: render 10,000+ media items without jank (pagination + virtualization where needed).
- Local search: <150ms on 100k records, offline.
- Bulk import: 1,000 items import/validate/dedup without freezing UI (background task with progress + cancel).
- No optimization without measurement: benchmarks in CI for search/insert/import/startup.

### 4.2 Security (NFR-SEC)
- Least-privilege Tauri capabilities: no blanket `fs`/`shell`/`http` access to the webview.
- API keys: never in source, never in git, never in logs; stored via OS keyring (fallback: encrypted local file).
- IPC inputs validated; SQL uses bind parameters only; no dynamic SQL from user input.
- Webview hardened: CSP, no remote content, no arbitrary protocol handlers.

### 4.3 Reliability & Integrity (NFR-REL)
- PRAGMA foreign_keys=ON, WAL mode, busy_timeout, transactions for multi-step writes.
- Versioned migrations run inside transactions; auto backup before migration; recovery path.
- Error handling: typed domain/infrastructure/provider/db errors with user-friendly messages.
- Logging: structured, no secrets/PII, debug log toggle, log rotation.

### 4.4 Offline (NFR-OFF)
- Full functionality offline; graceful handling of: no internet, API down, timeout, rate-limit,
  broken image URL, corrupt import, corrupt backup. Never loses data.

### 4.5 Maintainability (NFR-MNT)
- Layered architecture (Domain/Application/Infrastructure/Presentation); no god objects.
- Conventions: TypeScript strict, ESLint + Prettier, Rustfmt + Clippy, documented ADRs.
- Every schema change has a migration; docs updated per milestone.

### 4.6 Accessibility & i18n (NFR-ACC, NFR-I18N)
- WCAG 2.1 AA target for the UI shell; full keyboard operation; screen-reader labels.
- RTL as a first-class layout mode (logical properties), tested with Arabic + mixed LTR content.

---

## 5. Constraints

- Desktop application (Windows / macOS / Linux); primary target Windows.
- Stack mandated by product decision: **Tauri 2 + Rust backend + TypeScript frontend**.
- Database: **SQLite** (local single-file) with FTS5.
- Small user base (1 → small household). No accounts server, no multi-tenancy in scope.
- Must remain buildable by a single developer / small team; minimal dependency footprint.

## 6. Out of Scope (currently)

- Cloud sync, cloud account, hosted backend, web app.
- Streaming / playback of media content (tracker, not player).
- Social features, public profiles, recommendations engine, AI features.
- Plugin runtime, browser extension, mobile companion.
- Multi-user accounts on one device.
- Telemetry (explicitly off; opt-in, anonymous, documented, disable-able if ever added).

## 7. Priority Classification Used

`Core` = required for MVP · `Important` = high value, planned right after MVP · `Optional` = valuable but deferrable · `Future` = explicitly deferred (documented in `DECISIONS.md`).

## 8. Requirement Traceability (master table)

Full mapping lives in `DEVELOPMENT_PLAN.md`. Format:

```
REQ-TRACK-001  →  Feature: Episode/Chapter Tracking  →  TASK-120, TASK-121, TASK-122  →  TEST-TRACK-001
```
