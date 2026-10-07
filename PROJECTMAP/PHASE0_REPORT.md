# MyLore — Phase 0 Report (Discovery & Research)

> August 2026 · Final report of PHASE 0, ordered per spec §103. Each section points to the full
> artifact; this file is the executive hub.

---

## 1. Executive Summary

MyLore is a **local-first, offline-first, private desktop media tracker** (novels, web novels,
books, manga/manhwa/manhua, anime, TV, movies) built with **Tauri 2 (Rust) + TypeScript (React) +
SQLite (FTS5)**. Phase 0 produced: requirements, domain model, DB schema, architecture, verified
provider research, UX research, a design system, a milestone/task plan, and ADRs. Core
differentiator vs cloud trackers (MAL/AniList/Trakt) and self-hosted servers (Ryot/Yamtrack):
**single desktop binary, own local database, works fully offline, no account, no telemetry.**

**Key decisions (see §27):** sqlx-in-Rust for SQLite (ADR-002); unified `content_node` progress
tree (ADR-006); capability-based provider adapters — AniList, TMDB, MangaDex, OpenLibrary core,
Jikan/Google Books fallback (ADR-004/012); user-data/metadata separation (ADR-007); `.mylore`
backup/restore as core (ADR-008); React + TanStack Query + Zustand (ADR-003); en+ar with real RTL
(ADR-010); "Quiet Library" UI base (ADR-009). MVP = M1–M6 + basic providers + import/export +
reviews + backups (§24). 94 tasks across 13 milestones, all traceable to requirements (§28–30).

### ملخص تنفيذي (Arabic)

ميلور هو تطبيق سطح مكتب **محلي بالكامل** لتتبع ومتابعة الوسائط (روايات، كتب، مانجا/مانهوا/مانهوا،
أنمي، مسلسلات، أفلام)، مبني على **Tauri 2 (Rust) + TypeScript (React) + SQLite**. يعمل بدون إنترنت،
خصوصيته من التصميم، لا حساب ولا تتبع. انتهت مرحلة الاكتشاف والبحث: نموذج المجال، مخطط قاعدة
البيانات، المعمارية، بحث موثّق عن المصادر (AniList, TMDB, MangaDex, OpenLibrary)، تجربة المستخدم،
نظام التصميم، وخطة تطوير من 13 مرحلة و94 مهمة مع قاعدة بيانات محلية مصدراً واحداً للحقيقة.

---

## 2. Product Vision

A **personal local media management platform** (not a simple tracker, spec §106): calm, fast,
daily-use desktop app where the user's library is the source of truth and the internet is only a
metadata tap. `PROJECT_REQUIREMENTS.md §1`.

## 3. User Workflows

A) Daily tracking · B) Discover & add · C) Library management · D) Reflection (reviews/stats) ·
E) Safety (backup/import/export). `PROJECT_REQUIREMENTS.md §2`.

## 4. Functional Requirements

~40 ID'd requirements (REQ-MEDIA/Track/Review/Coll/Search/Import/Export/Backup/Dash/Stat/Cal/Ux/
Prov) with Core/Important/Optional priority. `PROJECT_REQUIREMENTS.md §3`.

## 5. Non-Functional Requirements

Performance (10k+ items, <150ms search, non-blocking UI) · Security (least privilege, keyring) ·
Reliability (FK/WAL/transactions/migrations) · Offline resilience · Maintainability (layering) ·
Accessibility (WCAG AA) · i18n (en/ar RTL). `PROJECT_REQUIREMENTS.md §4`.

## 6. Research Findings

Studied MAL, AniList, Kitsu (inactive 2026), Anime-Planet, Trakt, SIMKL, Letterboxd/Serializd,
Goodreads, StoryGraph, NovelUpdates, Hardcover, Bookwyrm, Bangumi, and self-hosted
Ryot/Yamtrack/Watcharr/Scrob + readers (Mihon, Kavita). Books/web-novel findings: Goodreads API
is dead → **CSV export is the import bridge**; StoryGraph = best-in-class book UX (mood/pace/
content warnings, DNF, stats) but no API; NovelUpdates = definitive WN/LN directory but no API
(ToS restricts scraping → the LNReader plugin, maintained by an NU moderator, publishes the
authoritative selectors we follow in MISSION-065 at a modest rate) → adopt its genre/tag taxonomy
+ Normal/Manual tracking modes, and its metadata via the adapter. Cross-cutting: status tabs,
≤2-step progress, filter-first libraries, import stories,
calendar/stats, and the gap — **no offline local option exists**; that is our wedge. `RESEARCH.md`.

## 7. Competitor Analysis

| App | Type | Strengths | Weaknesses | Borrowed idea |
|---|---|---|---|---|
| MAL | anime/manga web | scale, community, manga parity | dated UI, API gated, 2025 ownership change | status-tab nav, seasonal charts |
| AniList | anime/manga web | modern UI, free GraphQL API, relations | web-only | filter bar, card+progress overlay, API quality |
| Trakt | TV/movies | scrobbling, history, cross-ids | 2026 free caps | calendar, activity log, import path |
| Ryot | self-hosted | multi-media, imports, metadata wiring | server+Postgres, not offline | importer architecture, metadata providers |
| Yamtrack | self-hosted | per-season tracking, custom entries, calendar/iCal | server app | custom manual entries, per-season UX, CSV round-trip |
| Goodreads | books web | largest catalog, shelves, currently-reading | API dead, ads, no offline | CSV import bridge, shelves→collections |
| StoryGraph | books web | mood/pace/content-warning/DNF UX, reading stats | no API, social-first | mood/pace tags, reading stats, DNF-with-% |
| NovelUpdates | web novels/LN directory | unrivaled WN/LN taxonomy + release feed | no API; scrape selectors from the LNReader plugin (NU-moderator project) — MISSION-065 | WN/LN genre/tag taxonomy, Normal/Manual chapter modes + metadata adapter |
| Kitsu | anime/manga | (historic) | inactive 2026 | — (avoid) |

`RESEARCH.md` has full rows.

## 8. API Provider Analysis

Verified Aug 2026: **AniList** (free, no key, ~90 r/min, rich anime/manga + light novels
(MANGA/NOVEL) + cross-ids), **Jikan** (free MAL mirror, 3 rps/60 rpm), **MangaDex** (free public,
chapters incl. manhwa/manhua, must credit), **TMDB** (free non-commercial key, ~40 req/10 s,
attribution), **TVDB v4** (free tier), **Trakt** (free personal, 2026 caps, ~1 rps),
**OpenLibrary** (free, 1 rps, 3 rps identified), **Google Books** (free key, ~100 q/min),
**Hardcover** (free GraphQL book API, young), **Bangumi** (open CN ACGN API, ~1 rps),
**ISBNDB** (free 100 req/mo ISBN lookup), BookBrainz/Annict/SIMKL optional. Excluded: MAL official,
Kitsu, AniDB, Goodreads/Anime-Planet/StoryGraph (no usable public API); NovelUpdates is included via
MISSION-065's HTML-scrape adapter (no API — selectors from the LNReader plugin, an NU-moderator
project). `API_PROVIDERS.md`.

## 9. Technology Stack Decision

- Shell **Tauri 2** (ADR-001) · DB **SQLite + sqlx** (ADR-002) · UI **React+Vite+TS strict**
- Data/remote state **TanStack Query** · UI state **Zustand** · Forms **RHF+Zod**
- Routing **React Router 7** · Virtualization **TanStack Virtual** · Primitives **Radix** +
  **Tailwind** + design tokens · i18n **i18next** · Icons **Lucide**
- Rust: **tokio**, **reqwest**, **keyring**, **tracing** · Tests: **Vitest** + **cargo test** +
  **Playwright/tauri-driver** · CI: **GitHub Actions**.

## 10. Tauri 2 Architecture

Presentation (React) → typed IPC commands/events → Application services (Rust) → Domain (pure) ←
Infrastructure (sqlx repos, providers, fs, keyring). Long tasks = Tokio tasks emitting progress
events. Least-privilege capabilities; no blanket fs/shell/http in the webview. `ARCHITECTURE.md §1–2, §11`.

## 11. Domain Model

Single `Media` entity + contentType discriminator (composition over inheritance, tagged unions
rejected); generic `content_node` tree for all hierarchies; separate `tracking`/`review`/`tag`/
`collection` aggregates; `media_external_id` identity layer; merge with before-image. Invariants
listed. `DOMAIN_MODEL.md`.

## 12. Database Architecture

Full v1 DDL: 18 tables, PRAGMAs (FK/WAL), indexes, FTS5 (`unicode61` + `trigram` for CJK),
transaction-wrapped migrations, `.mylore` backup = `VACUUM INTO` + assets + meta. `DATABASE.md`.

## 13. Provider Architecture

Capability-based trait + normalization → unified domain metadata; `ProviderCoordinator` handles
rate limits, exponential backoff, timeouts, cancellation, error mapping; offline test fixtures.
`ARCHITECTURE.md §4`, `API_PROVIDERS.md §13`.

## 14. Import / Export Architecture

Pipeline: parser → validator → normalizer → deduplicator → **preview** → transaction → report;
bulk import preview required; export JSON/CSV/Markdown streaming. `ARCHITECTURE.md §6`.

## 15. Backup Architecture

`.mylore` archive, quarantine restore (never silent overwrite), auto-schedule + rotation,
pre-migration snapshot, corrupt-DB recovery prompt. `DATABASE.md §7`, `ARCHITECTURE.md §7`.

## 16. Search Architecture

Local FTS5 (multilingual) in Rust + external provider search via coordinator; combined result
model with "in library"/duplicate flags; identity-first import. `ARCHITECTURE.md §5`.

## 17. State Management Architecture

UI state (Zustand) · domain/remote (TanStack Query) · local (useState) · prefs
(tauri-plugin-store). Never sync Query into Zustand (two sources of truth). `ARCHITECTURE.md §3`.

## 18. UX Architecture

Left rail + status-filtered library + master–detail; quick-capture popover; dashboard widgets;
search-first command palette; states everywhere. `UX_RESEARCH.md §2–4`.

## 19. UI Design Directions

A "Quiet Library" (recommended base) · B "Media Dashboard" · C "Dense Power-User"; compared on
usability/density/a11y/RTL/maintenance → **A base + C affordances** (ADR-009). `UX_RESEARCH.md §6`.

## 20. Accessibility

Keyboard-first, focus management, WCAG 2.1 AA both themes, reduced-motion, Radix accessible
primitives, labels on all icons. `DESIGN_SYSTEM.md §11`, tasks TASK-037/087.

## 21. Internationalization / RTL

en+ar from M4; logical CSS properties + `dir` mirroring; per-title direction detection for
mixed/CJK/Arabic content; FTS normalization per script (ADR-010, REQ-UX-003).

## 22. Security

Least-privilege capabilities; API keys via OS keyring + encrypted fallback, redacted logs;
parameterized SQL only; hardened CSP; no devtools in release; shell opened only for allow-listed
provider URLs. `ARCHITECTURE.md §11`.

## 23. Performance

Targets: startup ~1s, 10k+ items virtualized, search <150ms on 100k, non-blocking bulk ops;
indexes + debounce + disk image cache; CI benchmarks (10k/50k/100k). `PROJECT_REQUIREMENTS.md §4.1`,
`DATABASE.md §5`, tasks TASK-021/088.

## 24. MVP Scope

M1–M6 + minimal providers (one anime, one book) + JSON/CSV import-export + reviews/tags + backups.
Concretely: Media CRUD · Library · Local search · Tracking · Basic provider import · Reviews ·
Tags · Backup/Restore. `DEVELOPMENT_PLAN.md §2`.

## 25. Future Scope

Deferred behind designed seams (not implemented): cloud sync (aggregates carry `updatedAt`;
conflict resolution at aggregate boundary), more providers/importers (Trakt/SIMKL), mobile
companion, plugins (provider adapter seam), AI features (optional, local, disable-able), games/
podcasts/music content types (data-only additions). Telemetry: none; if ever, opt-in/anonymous/
documented. `DECISIONS.md` ADR-013 + `PROJECT_REQUIREMENTS.md §6`.

## 26. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Provider API changes / downtime | external features degrade | 2 providers per type, coordinator retry, cached metadata, offline-first core |
| Licensing (TMDB/MangaDex attribution) | legal | attribution UI tasks tracked; MangaDex no-ads clause respected |
| CJK/Arabic search quality | search UX | FTS `trigram` + normalization columns + tests |
| Scope creep (social/sync/AI) | delivery | MVP gate, ADR-013, feature classification |
| Rust velocity | schedule | services are pure + tested; commands thin |
| WebView differences | rendering bugs | CSS-only layout, system fonts, E2E on 3 OSes |

## 27. Architecture Decisions

ADR-001 Tauri 2 · ADR-002 sqlx-in-Rust · ADR-003 React/Query/Zustand · ADR-004 capability
providers · ADR-005 FTS5 · ADR-006 node-tree progress · ADR-007 data/metadata separation ·
ADR-008 backup core · ADR-009 Quiet Library UI · ADR-010 i18n/RTL · ADR-011 keyring · ADR-012
provider set · ADR-013 scope discipline · Pending: SQLCipher, Trakt/SIMKL import, charts lib.
`DECISIONS.md`.

## 28. Milestones

M0 Research (this report) → M1 Foundation → M2 Database → M3 Domain → M4 UI Foundation → M5
Library MVP → M6 Tracking → M7 Providers → M8 Import/Export → M9 Reviews & Collections → M10
Stats & Calendar → M11 Backup & Recovery → M12 UX Polish → M13 Testing & Release.
`DEVELOPMENT_PLAN.md §1`.

## 29. Complete Task Breakdown

**100 missions** (MISSION-001…100, M1–M13) + future-scope (MISSION-101+), each with deps, priority,
files, tests, and acceptance criteria. Master mission list: `ROADMAP.md`; detailed reference:
`DEVELOPMENT_PLAN.md §3`.

## 30. Dependency Graph

Critical path: 001→…→020→022→039→040→047→058→059→063→…→093; parallel tracks (UI shell ‖ library;
provider adapters mutually independent; M10 ‖ M11). Full graph: `DEVELOPMENT_PLAN.md §4`.

## 31. Definition of Done

Implementation · Type safety · Tests · Error handling · UI validation · Docs · A11y (where needed)
· Perf (where needed) · no console/TS/lint errors · no broken deps. `DEVELOPMENT_PLAN.md §6`.

## 32. Quality Gates

TS PASS · Rust build PASS · Lint PASS · Tests PASS · Migrations PASS · Import/Export PASS ·
Backup/Restore PASS · Critical UX flows PASS · Security review PASS. `DEVELOPMENT_PLAN.md §7`.

## 33. Final Architecture Recommendation

Adopt this plan as-is: it is the **simplest architecture that satisfies the requirements** —
single-binary Tauri app, one SQLite source of truth, capability-based providers, unified
content-node tracking, strict metadata/user-data separation, backup as core, en/ar RTL UI built
on a token design system, and an incremental 13-milestone/94-task plan with quality gates.

**Next step (per protocol §90):** begin **M1 Foundation — TASK-001** (scaffold Tauri 2 + React +
TS), then proceed task-by-task with test → review → fix → docs → status updates. No code has been
written yet; no milestone is skipped; nothing is built ahead of its dependencies.

