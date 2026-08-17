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
    "banner_url": null
  }
]
```

**CSV import** (`CsvParser`) reads the file with a user-built `CsvMapping` (the MISSION-068 mapping
UI): each field names a CSV column; fields without a column stay `None` (the validator/normalizer
decide how to degrade the row). `default_content_type` applies one type to every row when the file
has no type column. `delimiter` is the CSV field delimiter, `separator` splits multi-value cells
(`alt_titles`, `genres`, `tags`, `external_id`). External-id cells must be `provider:value`
(e.g. `anilist:42`). Files are parsed `flexible`, so a ragged trailing line never aborts the batch.

- IPC: `import_file_preview` (parse + dedup → preview), `import_commit` (one transaction, savepoint
  per row; `plan` selects rows, null = every `New` row), `import_csv_headers` (column list for the
  mapping pickers). The webview picks the file (`<input type="file">` + `FileReader`) and passes the
  text — no fs/dialog plugin needed.
- `application::import_file_service` (`ImportFileService`) routes `json`/`csv` to the right parser,
  detects format by first byte, and reuses `ImportPipeline` for preview + savepoint commit.
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

## 7. Backup & Restore

`BackupService` in Rust: snapshot (VACUUM INTO) + assets manifest + meta.json → `.mylore` zip.
Restore validates, quarantines current data, swaps, verifies. Automatic scheduling + rotation.
(Full design: `DATABASE.md §7`.)

## 8. Background tasks

Unified `TaskManager` (**implemented**): every long operation (import, export, metadata sync, image
download, backup, migration, provider search) is a cancelable task with states
`queued → running(p) → success|failed|cancelled`, progress events to the UI, and a typed result.
`TaskManager` is managed as `Arc<TaskManager>`; its emitter forwards every change as a `task-changed`
event (payload `TaskSnapshot`). The import confirm command spawns a `TaskKind::ImportFile` task that
runs `ImportPipeline::commit_with_progress` inside `tokio::select!` against a cooperative cancel
flag. Cancellation propagates to Tokio tasks and HTTP requests (drop-based cancellation).

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
