# MyLore

**A local-first, offline-first, private media tracker.**
Track anime, manga, manhwa/manhua, novels, web novels and books — with real
chapter/episode progress engines, provider-powered discovery & import,
reviews, statistics, year-in-review recaps, a release calendar, collections
and safe backups. Your library lives in **one SQLite database on your
machine**; the internet is only ever used to *fetch* metadata.

- 🔒 **Private by design** — no account, no telemetry, no cloud. Encrypted-at-rest is opt-in on the roadmap.
- 🌐 **English + Arabic**, full RTL layout.
- 🧩 **10 metadata providers** (AniList, MangaDex, NovelUpdates, WTR-LAB, OpenLibrary, Bangumi, Jikan, TMDB, Google Books, Hardcover) with per-provider failure isolation — one down provider never breaks a search.
- 💾 **Portable backups** (`.mylore` archives), rollback-safe restore, pre-migration auto-backup.

---

## Table of contents

1. [For users](#for-users)
2. [Building from source](#building-from-source)
3. [Provider keys & the AniList client secret](#provider-keys--the-anilist-client-secret)
4. [Development workflow](#development-workflow)
5. [Repository map](#repository-map)
6. [Security](#security)

---

## For users

Download signed installers from the
[Releases page](https://github.com/OldOne094/MyLore/releases)
(Windows NSIS / macOS / Linux bundles are built by CI for every `v*` tag).
No installer? Any machine with Node 20+ and Rust can build it in two commands
(see below).

### What works without any account or key?

Everything local: library CRUD, tracking (status engine, chapter trees,
quick capture), reviews/tags/collections, search inside your own library,
statistics/recap/calendar, import from JSON/CSV (Goodreads, StoryGraph,
AniList exports), exports, and backups.

### What needs the internet?

Only provider operations: Discover search/import, cover downloads (cached
forever after first fetch), "refresh from provider", and NovelUpdates — which
runs through a tiny built-in browser window that handles Cloudflare for you
(it parks off-screen once connected).

---

## Building from source

### Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Node.js | ≥ 20 | frontend toolchain |
| Rust (stable) | ≥ 1.85 | backend; `wasm32` not needed |
| Platform deps for Tauri v2 | — | follow the official [prerequisites](https://tauri.app/start/prerequisites/) page for your OS |

### Commands

```bash
npm install          # frontend deps
npm run tauri dev    # run the desktop app with hot reload
npm run tauri build  # produce an installer for your OS

# quality gates (CI runs all of these)
npm run lint         # eslint
npm run format:check # prettier
npm run test         # vitest (frontend)
npm run codegen:check# IPC contract ↔ generated TS drift guard
cargo fmt --check
cargo clippy --all-targets -- -D warnings   # run inside src-tauri/
cargo test           # backend unit + integration tests
```

#### Optional: encrypt the database at rest (MISSION-112)

Build with SQLCipher and set a passphrase; when present, the library database
**and every `.mylore` backup made from it** are AES-encrypted:

```bash
# build with encryption support (needs perl + nasm + cmake, like Tauri's MSVC setup)
cargo build --features db-encryption

# run with a passphrase
MYLORE_DB_KEY="your long passphrase" npm run tauri dev
```

Losing the passphrase means losing the data — there is no recovery path by
design. Default builds without the feature stay plain SQLite and ignore the
variable entirely.

> The IPC boundary is generated: edit `scripts/ipc-contract.json`, then run
> `npm run codegen`. Never hand-edit `src/api/ipc.generated.ts`.

---

## Provider keys & the AniList client secret

MyLore is open source — **no credential of yours ever belongs in this
repository.** Everything below lives outside the repo, in your OS user
profile. Full policy: [`SECURITY.md`](SECURITY.md).

### Key-less providers (nothing to configure)

MangaDex, OpenLibrary, Bangumi and NovelUpdates work out of the box.

### Per-user API keys (paste them in Settings → Providers)

TMDB, Google Books and Hardcover each accept a personal key you paste into
the settings row. Keys are stored by the backend secret pipeline and are
never returned to the UI or written to logs.

### AniList — three ways, pick one

AniList works anonymously for search/details, but signing in raises rate
limits and removes intermittent 403 hiccups.

| Method | Steps |
|---|---|
| **A · Personal token (simplest)** | Create a token at <https://anilist.co/settings/developer> → *Personal tokens*, then paste it into the AniList row in Settings. Done. |
| **B · Browser sign-in (one-click)** | Register an API Client v2 on the same page with redirect URL `http://127.0.0.1:24110/auth/anilist/callback`, then place the **client secret** as described below and press *Sign in with AniList*. |
| **C · Environment variable** | Same registration, but expose the secret as `ANILIST_CLIENT_SECRET` when building/running. |

#### Where does the client secret file live?

Create a one-line text file named `anilist.client-secret` containing only the
secret, here:

| OS | Path |
|---|---|
| Windows | `%APPDATA%\com.mylore.app\anilist.client-secret` |
| macOS | `~/Library/Application Support/com.mylore.app/anilist.client-secret` |
| Linux | `~/.config/com.mylore.app/anilist.client-secret` |

Notes:

- The file must stay **outside the repository** — a guard test fails the
  build if a literal secret reappears in source (`SECURITY.md` §2).
- Without any secret, method A still works; only the browser sign-in button
  returns a clear error explaining exactly this.
- Because desktop apps cannot truly hide OAuth client secrets
  ([RFC 8252 §8.4](https://www.rfc-editor.org/rfc/rfc8252#section-8.4)),
  treat it as public knowledge and rotate it from the AniList developer page
  whenever you like.

---

## Development workflow

Work is organized as numbered missions tracked in
[`ROADMAP.md`](ROADMAP.md). Each mission follows
implement → test → review → docs → gates → status update. Milestone M0–M14
are complete (Alpha); current phase and future scope are at the top of the
roadmap.

Key docs:

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — layers, state, IPC policy engine
- [`API_PROVIDERS.md`](API_PROVIDERS.md) — every provider contract & quirks
- [`DATABASE.md`](DATABASE.md) / [`DOMAIN_MODEL.md`](DOMAIN_MODEL.md)
- [`DESIGN_SYSTEM.md`](DESIGN_SYSTEM.md) — tokens & primitives
- [`SECURITY.md`](SECURITY.md) — the secrets contract described above
- [`CHANGELOG.md`](CHANGELOG.md)

### Testing notes

Frontend suites mock the Tauri IPC layer (`@tauri-apps/api/core` +
`@tauri-apps/api/event`) globally via `src/test/setup.ts`; backend suites use
wiremock fixtures under `src-tauri/tests/fixtures/`. Live-network probes live
in `src-tauri/tests/live_network.rs` and are marked `#[ignore]`.

---

## Repository map

```
├─ src/                  # React + Tailwind v4 frontend (features/* slices)
│  ├─ api/               # query client, typed IPC wrappers (generated)
│  ├─ components/{shell,ui}/
│  └─ features/          # library, discover, stats, calendar, …
├─ src-tauri/            # Rust core (Tauri v2)
│  ├─ src/domain/        # pure business objects & rules (no IO)
│  ├─ src/application/   # services, coordinator/policy, task manager
│  ├─ src/infrastructure/# sqlx repos, provider adapters, images, keyring
│  └─ tests/             # integration suites + recorded fixtures
├─ e2e/                  # Playwright flows over a stubbed IPC bridge
├─ scripts/              # IPC codegen, release tooling
└─ ROADMAP.md            # mission tracker — start reading here
```

## Security

See [`SECURITY.md`](SECURITY.md). Short version: secrets live outside the
repo, tokens never cross the IPC boundary, logs carry lengths not values,
and a regression test keeps it that way.

## License

Not yet chosen — see [#license discussion]. Until a license lands, all rights
reserved by the authors; cloning to build a private copy for yourself is
explicitly fine.
