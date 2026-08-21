# Changelog

All notable changes to MyLore are documented here.
Format: [Keep a Changelog](https://keepachangelog.com), versions follow [SemVer](https://semver.org).

## [0.1.0-alpha.1] - 2026-08-21

### Added / Changed / Fixed

- test(providers): unified fixture harness + integrity guard for offline CI (MISSION-098)
- test(e2e): Playwright user-flow suite over stubbed IPC boundary (MISSION-097)
- test(backup): full-lifecycle backup/restore integration suite (MISSION-096)
- Refactor code for improved readability and consistency
- feat(ux): comfortable/compact density tiers in preferences (MISSION-095)
- perf: debounced type-ahead search + startup timing logs (MISSION-094)
- fix(a11y): WCAG AA contrast retune + regression test, pin aria-current (MISSION-093)
- fix(rtl): logical properties, keyboard direction inversion, mixed-direction titles (MISSION-092)
- test(states): audit all data surfaces, pin four untested state paths (MISSION-091)
- feat(ux): complete shortcut map, global add-title and shortcuts help (MISSION-090)
- feat(merge): merge UI with conflict preview + trash-based undo (MISSION-089)
- feat(backup): backups UI + recovery mode for corrupt database (MISSION-088)
- feat(backup): pre-migration auto-backup hook at startup (MISSION-087)
- feat(backup): automatic schedule + N+monthly rotation in preferences (MISSION-086)
- feat(backup): rollback-safe restore with quarantine + swap + verify (MISSION-085)
- feat(backup): .mylore backup service with VACUUM INTO snapshot + validation (MISSION-084)
- feat(reading): StoryGraph-style reading recap under Stats page (MISSION-083)
- feat(recap): year-in-review page of activity totals, chart and standouts (MISSION-082)
- feat(calendar): month grid of air dates + activity with day list (MISSION-081)
- feat(stats): stats service endpoints + stats page with charts (MISSION-080)
- feat(review): mood/pace/content-warning metadata with acknowledgment (MISSION-079)
- feat(library): bulk ops on filtered selection with summary (MISSION-078)
- feat(collections): smart collections from saved filters (MISSION-077)
- feat(collections): collections CRUD and drag/drop members (MISSION-076)
- feat(library): favorites flag in grid/list views (MISSION-075)
- feat(media): review/notes UI and personal-tag commands (MISSION-074)
- feat(import): import/export integration tests and fixtures (MISSION-073)
- feat(import): AniList / Goodreads / StoryGraph profile imports with user state (MISSION-072)
- feat(export): streaming JSON / CSV / Markdown export with save dialog (MISSION-071)
- feat(tasks): background TaskManager wired to import (MISSION-070)
- feat(import): preview + confirm UI (MISSION-069)
- feat(import): JSON + CSV file import with mapping UI (MISSION-068)
- feat(import): import pipeline core (MISSION-067)
- feat(providers): Bangumi adapter + fixtures (MISSION-066)
- feat(providers): Hardcover adapter + fixtures (MISSION-064)
- feat(settings): provider settings UI with keyring keys + connection tests (MISSION-063)
- feat(images): download/cache covers with broken-url handling (MISSION-062)
- feat(enrich): refresh provider metadata + diff report (MISSION-061)
- feat(import): import-from-provider flow with node tree (MISSION-060)
- feat(providers): NovelUpdates adapter + fixtures (MISSION-065)
- docs(roadmap): slot NovelUpdates as MISSION-065, shift 066-113, reverse NU ToS decision
- feat(discover): external search UI grouped by provider with identity flags (MISSION-059)
- feat(providers): Jikan + Google Books fallback adapters (MISSION-058)
- feat(providers): OpenLibrary adapter + fixtures (MISSION-057)
- feat(providers): MangaDex adapter + fixtures (MISSION-056)
- feat(providers): TMDB adapter + fixtures (MISSION-055)
- feat(providers): AniList adapter + fixtures (MISSION-054)
- feat(providers): provider trait + capabilities + coordinator (MISSION-053)
- feat(tracking): Normal/Manual tracking mode + DNF-with-progress (MISSION-052)
- feat(activity): implement activity logging for tracking actions and progress marks
- feat: implement dashboard feature with summary widgets
- feat: implement quick capture feature for marking media progress
- feat(progress): enhance progress tracking with new summary and next node functionality
- feat(tracking): implement tracking functionality for media status management
- feat(progress): implement per-node progress tracking with range marking
- feat: node tree endpoint + expand/collapse UI (MISSION-046)
- feat: implement bulk operations for media management
- feat: trash/restore UI + undo toast for deletes (MISSION-044)
- feat: local full-text search - header box + results page (MISSION-043)
- feat: media detail page - hero, meta tabs, tracking/review shells (MISSION-042)
- docs: mark MISSION-041 done + mission log entry
- feat: library filter panel, sort menu, group-by + facets endpoint (MISSION-041)
- feat: library landing view - grid/list/compact virtualization + view switcher (MISSION-040)
- refactor: M4 foundation fixes - type scale (text-md), switcher a11y roles, dead i18n key, shared locale labels
- feat: library MVP - manual add dialog, media create/list service + typed IPC (MISSION-038)
- feat: a11y baseline — skip link, semantic shell, screen-reader labels (MISSION-037)
- feat: command palette (Ctrl+Mod K) + shortcut registry (MISSION-036)
- feat: TanStack Query client, typed api.ts wrappers, query-key factory (MISSION-035)
- feat: persistent preferences via tauri-plugin-store + Settings page (MISSION-034)
- feat: i18n (i18next) en/ar, ICU plurals, RTL dir wiring, locale switcher (MISSION-033)
- feat: React Router shell with nav rail, topbar, status bar and empty-state pages (MISSION-032)
- feat: Tailwind v4 + Radix UI primitives (Button, field, Badge, Dialog, Popover, Toast, Skeleton) (MISSION-031)
- feat: design tokens, light/dark system themes, data-theme + theme provider (MISSION-030)
- test: service integration tests across progress, status, dedup, stats, merge (MISSION-029)
- feat: merge service with conflict report and before-image (MISSION-028)
- style: split joined statements in identity test (rustfmt)
- feat: stats service with counts, hours, completion and rating (MISSION-027)
- feat: identity service with exact and fuzzy matching (MISSION-026)
- feat: title normalization with script-aware fold and matching (MISSION-025)
- feat: status engine with explicit reversible transitions (MISSION-024)
- feat: progress engine with per-contentType templates (MISSION-023)
- fix: enforce LF for .mjs/.cjs/.mts in gitattributes (CI prettier)
- feat: domain types and invariant guards (MISSION-022)
- perf: database benchmarks for insert, search, bulk import (MISSION-021)
- test: db integration tests for cascades, fts, and transaction rollback (MISSION-020)
- feat: typed repository layer for all aggregates (MISSION-019)
- feat: fts5 search index + triggers + rebuild (MISSION-018)
- feat: user aggregates, asset + media cover/banner columns (MISSION-017)
- feat: tracking, seeded core statuses, node progress with CHECKs (MISSION-016)
- feat: media external id + relation tables with unique constraints (MISSION-015)
- feat: migrations + media schema + content-node tree (MISSION-012..014)
- feat: sqlx pool with pragmas + startup integrity check as managed state (MISSION-011)
- feat: window shell config + strict CSP (MISSION-010)
- feat: typed IPC boundary with codegen from contract (MISSION-009)
- fix: force LF for yaml files so prettier check passes on Windows CI
- feat: tracing logging to rolling files + AppError skeleton (MISSION-008)
- chore: scaffold Tauri 2 + React + TS foundation (M1)

## [0.1.0-alpha.1] - 2026-08-21

### Added / Changed / Fixed



