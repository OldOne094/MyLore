# MyLore — Architecture Decision Records

> Phase 0 · August 2026 · Status: ACCEPTED (unless noted)
> Format (spec §57): Context · Problem · Options · Decision · Consequences · Alternatives rejected.

---

## ADR-001 · Tauri 2 as the application shell

- **Context:** cross-platform desktop app (Windows primary), small binary, low memory, system
  webview, Rust backend for SQLite + network + fs.
- **Options:** Tauri 2 · Electron · Flutter Desktop · .NET MAUI.
- **Decision:** **Tauri 2** (stable; Rust backend; ~5–10 MB; capability-based security; mobile-ready).
- **Consequences:** frontend must respect WebView2/WebKit constraints; Rust learning curve; plugin
  ecosystem younger than Electron's.
- **Rejected:** Electron (size/memory), MAUI (Windows-centric), Flutter Desktop (different model,
  team skillset).

## ADR-002 · SQLite via `sqlx` in Rust — not `tauri-plugin-sql` from the webview

- **Context:** one local writer, migrations, FTS5, transactions, backup, single access path.
- **Options:** `tauri-plugin-sql` (SQL over IPC from JS); `sqlx` directly (async, managed state);
  `rusqlite` (sync).
- **Decision:** **`sqlx` directly in the Rust backend** with repositories; DB never exposed to JS.
- **Consequences:** more Rust code; single source of truth for SQL; easier transaction + FTS5 +
  backup control; no SQL injection surface from UI.
- **Rejected:** plugin-sql (scatters DB access, weaker control, still IPC), rusqlite (sync; we run
  async on Tokio).

## ADR-003 · Frontend: React + Vite + TanStack Query + Zustand + RHF/Zod

- **Context:** rich, long-lived desktop UI; clear separation of UI state, domain data, prefs.
- **Options:** React vs Svelte vs Solid vs Vue; Redux vs Zustand/Jotai; raw fetch vs Query.
- **Decision:** **React 18/19 + Vite + TS strict**; **TanStack Query v5** for domain/remote data
  (cache, invalidation, optimistic updates); **Zustand** for UI state; **React Hook Form + Zod**
  for forms; **React Router v7**; **TanStack Virtual** for 10k+ lists; **Radix + Tailwind**.
- **Consequences:** mainstream ecosystem, hiring, types; keep Query vs Zustand split discipline.
- **Rejected:** Redux (boilerplate), Svelte/Solid (smaller ecosystem, team familiarity), raw fetch
  everywhere (no cache/invalidation).

## ADR-004 · Provider architecture: capability-based adapters behind a coordinator

- **Context:** many metadata sources, heterogeneous capabilities, rate limits, licensing.
- **Options:** one monolithic fetcher; per-provider adapters w/ shared normalization; capability
  interface.
- **Decision:** provider **trait + `ProviderCapabilities`**, normalization into unified domain
  metadata, `ProviderCoordinator` for rate-limit/retry/timeout/cancel/error mapping. New provider
  = new adapter only (spec §6, §44).
- **Consequences:** extra abstraction is justified (real multiple implementations, spec §84); each
  adapter testable with recorded fixtures offline.
- **Rejected:** hard-wired API calls in services; a generic "one size fits all" fetcher.

## ADR-005 · Search: SQLite FTS5 with multilingual tokenization

- **Context:** local full-text search over titles/alt titles/authors/genres/tags/notes/reviews.
- **Options:** FTS5; LIKE on titles (fine for prefix, fails otherwise); separate engine (Lunr,
  TinySearch, Meilisearch).
- **Decision:** **FTS5** (`unicode61` for Latin/Arabic + normalization; `trigram` for CJK
  substring), maintained index + triggers, BM25 ranking (spec §16).
- **Consequences:** zero new infra; needs careful Arabic diacritic folding and CJK column strategy;
  no fuzzy typo-tolerance (accepted; prefix search suffices).
- **Rejected:** embedded engines (dependency + index duplication), LIKE-only (no ranking/scaling).

## ADR-006 · Progress model: generic `content_node` tree + derived aggregates

- **Context:** books(pages), novels/manga(chapters, volumes), anime/TV(episodes, seasons), movies.
- **Options:** inheritance per type; per-type tables; unified node tree.
- **Decision:** **single `content_node` tree** (kind+parent+position) with uniform per-node progress;
  aggregates computed (never stored). Media type only selects a *progress template*.
- **Consequences:** new media types are data; no "hacks" for TV seasons vs manga volumes
  (spec §11/§12); queries slightly more complex (tree walk).
- **Rejected:** inheritance tables (SQLite-hostile), separate episode/chapter tables (duplication,
  harder future types).

## ADR-007 · User data vs metadata separation (local-first data ownership)

- **Context:** provider refresh must never clobber progress/notes/ratings (spec §74/§75).
- **Options:** mixed single table; separate aggregates.
- **Decision:** metadata (`media`, nodes) and user data (`tracking`, `review`, collections,
  personal tags, activity) are **separate aggregates**; enrichment writes only metadata with a
  diff report.
- **Consequences:** clean boundaries; migration/sync-safe; slight join cost.
- **Rejected:** mixed row (metadata refresh could overwrite user fields).

## ADR-008 · Backup/restore as core, `.mylore` archive, quarantine restore

- **Context:** data loss is unacceptable (spec §21, §53).
- **Options:** copy db only; zip snapshot; db + assets + meta archive.
- **Decision:** `.mylore` = SQLite snapshot (`VACUUM INTO`) + assets + `meta.json` + checksums;
  restore validates, quarantines current data (never silent overwrite), swaps, verifies; auto
  backup before migrations + scheduled rotation.
- **Consequences:** first-class Backup epic in M11; simple user model ("one file to back up").
- **Rejected:** raw db copy (no assets, inconsistent), cloud backup (offline-first).

## ADR-009 · UI direction: "Quiet Library" (Design A) + progressive power features

- **Context:** daily-use desktop tracker; must stay fast/calm, yet dense for power users (spec §80).
- **Options:** A Quiet Library · B Media Dashboard (cinematic) · C Dense Power-User.
- **Decision:** base on **A**; layer C affordances (shortcuts, command palette, bulk, filters,
  compact density) as features. B-style cinematic covers allowed as optional theming only.
- **Consequences:** lower art risk; polish budget focused on micro-interactions and states.
- **Rejected as default:** B (heavy art, drift risk), C (cramped, poor discovery/RTL comfort).

## ADR-010 · i18n: English + Arabic, RTL by design, FTS-aware

- **Context:** bilingual product from day one; mixed-direction and CJK titles (spec §38).
- **Options:** add i18n later; en-only now; full RTL from start.
- **Decision:** **en + ar from M4**; logical CSS properties + `dir` mirroring; title text keeps its
  own script direction; search normalizes per-script.
- **Consequences:** layout discipline required early (no hard-coded left/right); all components
  tested in RTL.
- **Rejected:** delayed i18n (retrofit cost), en-only (excludes primary audience).

## ADR-011 · API keys in OS keyring; no secrets in source/logs

- **Context:** TMDB/Trakt/etc. keys; privacy + least privilege (spec §46).
- **Options:** env vars; plain settings JSON; OS keyring; encrypted file.
- **Decision:** **OS keyring** (`keyring` crate) with encrypted-file fallback; UI to add/test/remove;
  redaction in all logs.
- **Consequences:** small native dependency; clear user control; no git leakage by construction.
- **Rejected:** plain file (plaintext at rest), env-only (bad UX for end users).

## ADR-012 · Providers chosen: AniList + TMDB + MangaDex + OpenLibrary (core), Jikan + Google Books (fallback)

- **Context:** free, legal, rate-limitable sources per content type (verified Aug 2026).
- **Decision:** core spine per type with cross-fallbacks; identity via cross-provider external ids.
- **Consequences:** no single point of failure; attribution obligations (TMDB/MangaDex) tracked as
  UI tasks.
- **Rejected:** MAL official API (approval gating, 2025 ownership change), Kitsu (inactive), AniDB
  (no open API).

## ADR-013 · Scope discipline: personal/local MVP; no social, no server, no telemetry

- **Context:** tracker scope creep is the top failure mode (spec §59, §81).
- **Options:** build social/sync/AI now; defer with seams.
- **Decision:** **MVP is personal & local**; sync/social/AI/plugins are Future behind designed
  seams (aggregates carry `updated_at`; provider adapters are the plugin seam) — not implemented now.
- **Consequences:** simpler, safer core; ADR explicitly documents deferred scope.
- **Rejected:** early multi-user/cloud (cost without evidence of need).

## Open decisions (need evidence before closing)

- **ADR-PEND-1 · SQLCipher:** only if encryption requirement appears; costs native sqlite build.
- **ADR-PEND-2 · Trakt/SIMKL as import providers:** after M8 proves import pipeline.
- **ADR-PEND-3 · Charts library:** hand-rolled SVG vs a tiny chart lib — decided at M10 after
  measuring bundle/effort.

---
