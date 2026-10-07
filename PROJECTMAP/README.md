# PROJECTMAP — the project's documents

Start here. Everything in this folder is _about_ the code; the code itself lives
in `src/` (React/TypeScript), `src-tauri/` (Rust) and `e2e/` (Playwright, over a
stubbed IPC boundary).

## Reading order

1. **[PROJECT_REQUIREMENTS.md](./PROJECT_REQUIREMENTS.md)** — what we are building and why, with priorities.
2. **[ROADMAP.md](./ROADMAP.md)** — the master list: milestones, every mission, its status. _A mission is done when its row says so, and not before._
3. **[ARCHITECTURE.md](./ARCHITECTURE.md)** — layers, state ownership, the IPC policy engine.
4. **[DOMAIN_MODEL.md](./DOMAIN_MODEL.md)** — the entities and the invariants they carry.
5. **[DATABASE.md](./DATABASE.md)** — schema, migrations (each one tabulated) and the backup/restore contract.
6. **[DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md)** — tokens, primitives, motion.

## Roles

| Doc | Role |
|-----|------|
| **ROADMAP.md** | Master: milestones + the complete mission list + status. |
| DEVELOPMENT_PLAN.md | Reference: per-task detail (files, tests, AC) + traceability. |
| PROJECT_REQUIREMENTS.md | What we build (requirements, priorities). |
| PROJECT_MAP.md | Where things live in the tree. |
| PHASE0_REPORT.md | Executive summary of Phase 0. |
| RESEARCH.md · API_PROVIDERS.md · UX_RESEARCH.md | Why we build it that way. |
| ARCHITECTURE.md · DOMAIN_MODEL.md · DATABASE.md · DECISIONS.md · DESIGN_SYSTEM.md | How. |
| SECURITY.md | The secrets contract, the threat model, and what is deliberately out of scope. |
| TESTING.md | Test pyramid, fixtures, the E2E-over-stub rationale, benchmarks, the CI gates. |
| RELEASING.md | How a release is cut and published. |
| CHANGELOG.md | What shipped, per version. |
| MILESTONE-REPORT.md | Per-milestone release evidence. |
| Audit_Report_Claude.md | An audit of the codebase contributed to the project; it is the input the M20/M21 missions came out of. |

## Working on a mission

`ROADMAP.md §4` is the workflow — pick up a READY mission, implement, run the
gates in `ROADMAP.md §5`, set the status, and append a log line under the row
saying what was actually done (including anything that was deliberately left
out). Three rules worth repeating here, because they are expensive to get wrong:

- **Never edit an applied migration.** sqlx records each file's SHA-384 and
  refuses to open a database whose recorded copy differs, so editing one in
  place does not migrate anything — it stops every existing install from
  launching. Add a new migration and append its checksum to
  `MIGRATION_CHECKSUMS`; the guard test will tell you if you forget.
- **A green suite is not the same as a working app.** `TESTING.md` lists the
  guards that exist precisely because something shipped broken while every test
  passed.
- **Follow the conventions of the layer you are in.** The Rust side is layered
  (domain → application → infrastructure → commands); the frontend keeps one
  source of truth per concept and normalises IPC payloads at the query boundary.
