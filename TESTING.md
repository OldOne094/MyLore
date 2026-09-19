# MyLore — Testing Strategy

> Canonical home for how MyLore is tested (MISSION-149). Referenced by
> `ARCHITECTURE.md` (§4 provider fixtures, §9 error handling) and `PROJECT_MAP.md`.
> Requirements: `PROJECT_REQUIREMENTS.md` §99–100; release gates: `ROADMAP.md` §5.

---

## 1. Test pyramid

| Layer | Tool | Where | Count (approx., 2026-09) |
|-------|------|-------|--------------------------|
| Rust unit | `cargo test --lib` | inline `#[cfg(test)] mod tests` | ~690 |
| Rust integration | `cargo test --test …` | `src-tauri/tests/` | 15 |
| Frontend unit/component | Vitest + Testing Library | `src/**/*.test.{ts,tsx}` | ~330 across ~50 files |
| End-to-end | Playwright | `e2e/` | 5 flows |
| Benchmarks | `cargo bench` | `src-tauri/benches/` | `database.rs` |

The pyramid is deliberately bottom-heavy: business rules, dedup, status transitions, import/export
and backup/restore are validated in Rust; components are validated in jsdom; only whole user flows
reach Playwright.

---

## 2. Rust: unit tests

- **Domain** is pure and DB-free — its unit tests need no fixtures. Examples: `normalize` (12),
  `status` (11), `progress` (10), `identity` (9), `stats` (7), `review` (7), `media` (5).
- **Application services** take a `SqlitePool`; unit tests build a **file-backed migrated DB** via
  `infrastructure::test_support::{migrated_pool, temp_db_path, cleanup_files}`.
- **Infrastructure repositories** are integration-tested against the same migrated DBs, so SQL
  (joins, FTS, cascades, transactions) is exercised for real — never mocked.
- Run: `cd src-tauri && cargo test` (CI runs this on Linux/macOS/Windows).

## 3. Rust: integration tests (`src-tauri/tests/`)

| File | Covers |
|------|--------|
| `domain_services.rs` | Status lifecycle, stats math, dedup → merge → re-parenting, Arabic/title variants (pure domain, in-memory). |
| `import_export.rs` | `ImportFileService::detect`/`commit` + `ExportService` over real migrated pools; fixture detection for every source kind; JSON round-trip. |
| `backup_restore.rs` | Full production backup → mutate → restore path, retention across sessions, tampered-archive rejection. |
| `live_network.rs` | **`#[ignore]`d** real-HTTP probes for provider adapters — run manually (`cargo test -- --ignored`), never in CI. |
| `network_diag.rs` | **`#[ignore]`d** ad-hoc connectivity diagnostics. |
| `integration_tests.rs` | Small transactional/atomicity helpers (module, no `#[test]`s of its own). |

Fixtures live under `src-tauri/tests/fixtures/` (`import/`, `<provider>/`). They double as sample
files for the Import dialog.

## 4. Provider adapters: offline fixtures (never real network)

Every adapter test serves **recorded responses** from `src-tauri/tests/fixtures/<provider>/`
through in-process **wiremock** servers with an injected base URL. The suite therefore never
touches the network.

- Shared harness: `infrastructure::test_support::fixture(provider, name)` plus `mount_get` /
  `mount_post` helpers (`providers::test_support`).
- A **fixture-integrity test** (`all_committed_fixtures_are_intact`) walks the corpus asserting
  every file is non-empty and parses as JSON, so truncated/hand-edited recordings can't land.
- Adapters are also exercised through the **real coordinator** (`works_under_the_coordinator`) to
  prove rate-limit/retry/backoff policy applies.

This is the mechanism `ARCHITECTURE.md §4` refers to when it says fixtures "enable offline tests".

## 5. Frontend: unit & component tests (Vitest)

- Config: `vite.config.ts` (`environment: "jsdom"`, `setupFiles: ./src/test/setup.ts`). The `e2e/`
  and local `.freebuff/**` scratch trees are excluded.
- **IPC is mocked globally**: `src/test/setup.ts` mocks `@tauri-apps/api/core` (`invoke`) and
  `@tauri-apps/api/event` (`listen`/`emit`), so components are tested against scripted command
  responses, not a live backend.
- `src/test/setup.ts` also installs a `ResizeObserver` polyfill (Radix/virtualizer) and reports a
  viewport so TanStack Virtual renders in jsdom.
- Conventions: one `*.test.tsx` per feature/page; assert the four data states (loading skeleton,
  empty, error+retry, content) where a surface shows data; keep i18n parity covered by
  `src/i18n/locales.test.ts` (exact key match EN↔AR, Arabic superset of plural forms).

## 6. End-to-end (Playwright)

- `npm run e2e` drives the **real React tree** over the Vite dev server in the installed Chromium/
  Edge channel, **with the Tauri IPC boundary stubbed pre-boot**: `e2e/ipc-stub.ts` replaces
  `window.__TAURI_INTERNALS__` (per-command fixtures, invocation recording, faked
  event/store/dialog plugins).
- **Why a stub, not `tauri-driver`:** WebView2 exposes no WebDriver endpoint compatible with
  Playwright, so driving the actual webview is not possible in CI. The suite drives the same
  renderer users see across a scripted boundary instead — deterministic and offline.
- Flows (`e2e/flows.spec.ts`): add media, library search, track progress, import a file, and
  backup/restore through the guarded dialog.

## 7. Benchmarks

- `src-tauri/benches/database.rs` (Criterion) measures search latency, insert, bulk import and
  query plan behaviour. Run manually with `cargo bench`.
- **Known gap (Beta gate):** CI does **not** yet run `cargo bench` with thresholds — NFR-PERF
  benchmark enforcement is promised for Beta in `MISSION-100`. The `MILESTONE-REPORT.md` release
  gates track this.

## 7.1 Release size budget (`npm run size:budget`)

`scripts/binary-size.mjs` (MISSION-118) builds the app twice in release mode — default, then
`--features p2p` — and fails when either binary exceeds its budget or when the optional feature
adds more than its allowance:

| Build | Budget |
|-------|--------|
| default | 40 MB |
| `--features p2p` | 52 MB |
| p2p delta | 14 MB |

Why it exists: an optional feature is only honest if its cost is visible. The check catches a sync
dependency leaking into the default build and the `p2p` feature growing the binary unnoticed. It
compiles from scratch, so it is a **release gate**, not part of the everyday loop — run it before
tagging, alongside the other gates.

## 8. Quality gates (what must pass)

CI (`.github/workflows/ci.yml`) runs on every push/PR, on **ubuntu / macos / windows**:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
npm run lint          # eslint
npm run codegen:check # IPC contract ↔ generated TS drift guard
npm run format:check  # prettier
npm test              # vitest
npm run build         # tsc + vite build
```

Local parity: `npm run e2e` for the Playwright flows (not wired into CI), and
`cargo test -- --ignored` for the live-network probes (manual only).

Two guards exist because a green suite is not the same as a working app (both came out of a
startup failure that no test could see):

- **`applied_migrations_are_never_edited`** (`infrastructure/db.rs`) pins the SHA-384 of every
  embedded migration. sqlx rejects a database whose recorded checksum differs, so editing an
  applied migration makes the app fail to start for every existing install — this test turns that
  into a `cargo test` failure. Adding a migration means appending its checksum to the table.
- **`init_is_idempotent_and_writes_log_files`** (`infrastructure/logging.rs`) also asserts that a
  line logged before `logging::shutdown()` is on disk, which is what the fatal startup path relies
  on: a release build has no console, so an unlogged failure is an invisible one.

## 9. Conventions & gotchas

- **Never edit an applied migration**; add a new one (see `DATABASE.md §6`).
- Prefer asserting observable behaviour (Rust: return values + DB state; TS: rendered text/roles).
- Use `mutateAsync().then()` (not per-call `onSuccess`) when a mutation's toast must survive the
  component unmounting — React Query v5 drops per-call callbacks on unmount.
- jsdom quirks: Radix needs the `ResizeObserver` polyfill; virtualized lists need a reported
  viewport; `dataTransfer`-free drag-and-drop is driven with `fireEvent`.
- Tests must be hermetic: unique temp paths per test (see `temp_db_path`/`temp_settings_file`) —
  shared fixed paths race under parallel execution.
