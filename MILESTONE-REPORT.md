# MyLore Milestone Report

**Version:** 0.1.0-alpha.1 · **Date:** 2026-08-21 · **Phase:** M13 complete — Alpha gate reached.

This report closes the Phase-0 roadmap (M0–M13). Each milestone's exit criterion is listed
with its verification evidence. The release-gate verdicts at the bottom map directly to
DEVELOPMENT_PLAN.md §7 ("Quality gates before any release") and PROJECT_REQUIREMENTS.md §4.

---

## 1. Release gates (spec §85)

| Gate | Verdict | Evidence |
|------|---------|----------|
| TypeScript | **PASS** | `tsc --noEmit` strict, 0 errors; ESLint 0 errors |
| Rust build | **PASS** | `cargo build` clean; clippy `-D warnings` clean in CI on 3 platforms |
| Lint | **PASS** | ESLint + rustfmt + prettier enforced by husky pre-commit and CI |
| Tests | **PASS** | 683 Rust (668 lib + 15 integration) + 315 Vitest suites + 5 Playwright E2E flows — all green |
| Migrations | **PASS** | 12 versioned migrations; sqlx wraps each in a transaction; startup verifies integrity + applies pending atomically |
| Import/Export | **PASS** | Integration suite covers fixture detection across all source kinds, profile persistence, JSON round-trip through the importer, CSV/Markdown exports |
| Backup/Restore | **PASS** | Full-lifecycle integration test: create → mutate → restore returns the exact pre-mutation world; tampered archives rejected; retention policy enforced |
| Critical UX flows | **PASS** | Five Playwright journeys (add, search, track, import, backup/restore) run against the real renderer on every `npm run e2e` |
| Security review | **PASS*** | No network in webview capabilities; keys only in OS keyring; SQL bind params everywhere; CSP locked; least-privilege Tauri ACLs. (*external audit still recommended before Stable — see §4) |

## 2. Milestones

| MS | Exit criterion | Status | Key evidence |
|----|----------------|--------|--------------|
| M0 | Research & design docs | **DONE** | PHASE0_REPORT, ROADMAP, REQUIREMENTS, ARCHITECTURE, DESIGN_SYSTEM, DATABASE, DOMAIN_MODEL |
| M1 | Tauri 2 builds Win/mac/Linux; typed IPC skeleton | **DONE** | CI matrix green on 3 platforms; codegen-locked IPC contract (`ipc-contract.json` → generated TS) |
| M2 | SQLite via sqlx: schema, repos, FTS5 | **DONE** | 12 migrations; FTS unicode61+trigram; repo tests over migrated DBs |
| M3 | Domain layer pure + tested | **DONE** | tracking/status/progress/identity/stats/merge engines; property-style unit suites |
| M4 | Design tokens, shell, router, i18n en/ar | **DONE** | Token CSS consumed everywhere; hash router; RTL mirror pass (092); a11y AA contrast pinned by regression test (093) |
| M5 | Library MVP | **DONE** | CRUD, grid/list/compact virtualized views, filters/sort/facets, detail page, trash/undo |
| M6 | Tracking engine + dashboard | **DONE** | Node trees, per-node progress, Normal/Manual mode, quick capture, status auto-transitions |
| M7 | Providers | **DONE** | 10 providers behind a capability seam; enrich diff flow; recorded-fixture offline harness (098) |
| M8 | Import/Export | **DONE** | Goodreads/StoryGraph CSV, AniList/MAL/MyLore JSON, CSV mapping UI; JSON/CSV/Markdown export |
| M9 | Reviews & Collections | **DONE** | Reviews w/ mood-pace-warnings metadata, favorites, smart + manual collections, bulk ops |
| M10 | Stats & Calendar & Recap | **DONE** | Stats page, StoryGraph reading recap, air-date calendar, year-in-review |
| M11 | Backup & Recovery | **DONE** | .mylore archives (VACUUM INTO + assets), rollback-safe restore, schedule + N/monthly rotation, pre-migration hook, Backups UI + corrupt-DB recovery screen, merge UI with conflict preview + trash undo (084–089) |
| M12 | UX Polish | **DONE** | Complete shortcut map, states audit, RTL pass, WCAG AA contrast pinned, perf pass (debounced type-ahead, startup timing), density tiers (090–095) |
| M13 | Testing & Release | **DONE** | Integration suites (096), E2E (097), offline provider harness (098), release pipeline (099), this report (100) |

## 3. Metrics snapshot

| Metric | Value |
|--------|-------|
| Rust tests | **683** (668 lib incl. 224 provider, 15 integration across 3 targets) |
| Frontend unit tests | **315** (Vitest, jsdom) |
| E2E flows | **5** (Playwright, Edge channel, stubbed IPC boundary) |
| IPC commands under contract | ~60, codegen-enforced (`npm run codegen:check`) |
| DB migrations | 12 (+ seeds), all transactional |
| Provider fixtures committed | 10 providers / 24 recordings, integrity-guarded |
| Dependencies added for M13 | @playwright/test (dev-only) |

## 4. Release stages

### Alpha — ✅ REACHED (0.1.0-alpha.1)
All thirteen milestones shipped behind per-mission gates. Core user journeys are exercised
end-to-end on every commit (CI) and every e2e run. Installers build unsigned from the tag
pipeline. Known limitations are tracked as FX missions (cloud sync, plugins, AI, mobile).

### Beta — criteria (not yet)
- Signing certificates provisioned → installers signed/notarized on Windows + macOS.
- Benchmarks wired into CI enforcing NFR-PERF budgets (startup ≈1s window, search <150ms
  @100k, 10k-item library without jank).
- Soak period: ≥2 weeks of daily-driver use across Windows + one non-Windows platform with
  zero data-loss reports.
- External security review of the Tauri capability set and keyring usage.
- Arabic copy proofread by a native speaker (UI strings are translated but unreviewed).

### Stable — criteria (after Beta)
- Beta criteria held for one full release cycle.
- Crash-free sessions ≥99.5% (telemetry stays OFF — measured opt-in or manually).
- Documentation set current: README quickstart verified against a fresh install from the
  signed artifact; RELEASING.md exercised end-to-end once more.

---

*Milestone reports are cumulative: this file is updated at each release tag rather than
replaced.*
