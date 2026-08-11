# MyLore — Project Map

> Phase 0 · August 2026 · Planned repository layout + documentation index.
> Note: structure is the *target*; it materializes with EPIC-001/002.

## 1. Documentation index

| Doc | Purpose |
|-----|---------|
| `PHASE0_REPORT.md` | Final Phase-0 report (33-section summary, decision hub). |
| `PROJECT_REQUIREMENTS.md` | Vision, FR/NFR, constraints, out-of-scope, priorities. |
| `DOMAIN_MODEL.md` | Entities, invariants, identity/dedup, services. |
| `DATABASE.md` | SQLite schema (v1 DDL), FTS5, migrations, backup. |
| `ARCHITECTURE.md` | Layering, Tauri split, frontend, providers, pipelines, security. |
| `API_PROVIDERS.md` | Verified provider research + matrix + risks. |
| `RESEARCH.md` | Competitor analysis + positioning. |
| `UX_RESEARCH.md` | UX patterns, page specs, design directions. |
| `DESIGN_SYSTEM.md` | Tokens, components, themes, RTL, a11y. |
| `DEVELOPMENT_PLAN.md` | Milestones, epics/tasks, deps, DoD, quality gates, traceability. |
| `DECISIONS.md` | ADRs (accepted + pending). |
| `TESTING.md` | Test strategy (unit/integration/E2E, provider mocks, benchmarks). |

## 2. Repository layout (target)

```
MyLore/
├─ docs/                    # the documentation set above (lives at repo root during Phase 0)
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs / lib.rs   # Tauri builder, plugins, capabilities, setup
│  │  ├─ commands/          # thin IPC handlers (media, tracking, search, providers, import,
│  │  │                     #   export, backup, settings, stats, collections, trash)
│  │  ├─ domain/            # entities, value objects, invariants (pure)
│  │  │  ├─ media.rs · content_node.rs · tracking.rs · review.rs · identity.rs
│  │  │  ├─ status.rs · progress.rs · merge.rs · stats.rs
│  │  ├─ application/       # services / use-cases
│  │  │  ├─ media_service.rs · tracking_service.rs · search_service.rs
│  │  │  ├─ import_service.rs · export_service.rs · backup_service.rs
│  │  │  ├─ provider_coordinator.rs · collection_service.rs
│  │  ├─ infrastructure/
│  │  │  ├─ db.rs           # pool, pragmas, integrity
│  │  │  ├─ migrations/     # versioned SQL (sqlx)
│  │  │  ├─ repositories/   # media_repo.rs · node_repo.rs · tracking_repo.rs · ...
│  │  │  ├─ providers/      # anilist.rs · tmdb.rs · mangadex.rs · openlibrary.rs
│  │  │  │                 # jikan.rs · googlebooks.rs · normalize/ · fixtures/
│  │  │  ├─ image_cache.rs · backup.rs · keyring.rs · logging.rs
│  ├─ capabilities/         # least-privilege permission files
│  ├─ tauri.conf.json
├─ src/                     # React frontend
│  ├─ main.tsx · app.tsx · router.tsx
│  ├─ api/                  # typed command/event wrappers + query keys
│  ├─ features/
│  │  ├─ library/ · media-detail/ · tracking/ · search/ · discover/ · dashboard/
│  │  ├─ collections/ · reviews/ · stats/ · calendar/ · import-export/ · backups/
│  │  ├─ settings/ · providers/ · command-palette/
│  ├─ components/ui/        # design-system primitives
│  ├─ design-tokens/ · themes/
│  ├─ i18n/ (en.json, ar.json)
│  ├─ stores/ (zustand UI state)
│  ├─ hooks/
├─ tests/
│  ├─ unit/  (vitest)
│  ├─ fixtures/ (provider recordings, imports, backups)
│  └─ e2e/   (playwright + tauri-driver)
├─ scripts/                 # codegen (IPC types), benchmarks, release
├─ package.json · vite.config.ts · tsconfig.json · eslint.config.js
├─ Cargo.toml
```

## 3. Naming & conventions

- Crate: `mylore`; product identifier `com.mylore.app` (decide at packaging).
- Rust modules follow the four layers; commands are thin, services hold logic (spec §83).
- Frontend: feature folders; no business logic in components; IPC types generated/shared.
- Commits: `feat: fix: refactor: test: docs: chore: perf:` (spec §69).

## 4. Environments & data paths

- DB + logs + image cache: platform app-data dir (`%APPDATA%/mylore` on Windows; `~/.local/share`
  Linux; `~/Library/Application Support` macOS). Backups default to a user-chosen location.
