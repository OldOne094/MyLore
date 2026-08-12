# MyLore — Master Roadmap

> Phase 0 · Master roadmap · August 2026
> **This is the single source of truth for what we build and in what order.**
> Milestones first (§2), then the complete mission list (§3).
> Detailed per-mission implementation notes, files, tests and acceptance criteria stay in
> `DEVELOPMENT_PLAN.md` (the reference). Requirements in `PROJECT_REQUIREMENTS.md`.

---

## 1. Product in one line

A **local-first, offline-first, private desktop media tracker** (novels, web novels, light
novels, books, manga/manhwa/manhua, anime, TV, movies) — single binary (Tauri 2), own SQLite
database, no account, no ads, no telemetry. The internet is only a metadata tap; the library is
the source of truth. (`PROJECT_REQUIREMENTS.md`, `PHASE0_REPORT.md`.)

**Missions** are the smallest actionable units. Each mission has: ID · Title · Goal · Deps ·
Files · Tests · Acceptance Criteria · Complexity (S/M/L) · Priority · Status. When picked up, its
full checklist is created in the epic folder (`DEVELOPMENT_PLAN.md §5`).

---

## 2. Milestones

Milestones are the phases. We never build ahead of a milestone's exit criteria (spec §61).

| MS | Name | Exit criterion | Deps |
|----|------|----------------|------|
| M0 | Research & Design | This document set (Phase 0) incl. roadmap. | — |
| M1 | Foundation | Tauri 2 app builds on Win/macOS/Linux; TS strict + lint + fmt + CI green; typed IPC skeleton; empty window shell. | M0 |
| M2 | Database | SQLite via sqlx: migrations, full schema, repositories, FTS5, integrity pragmas; tests green. | M1 |
| M3 | Domain Layer | Domain entities/services in Rust with unit tests (tracking math, dedup, status transitions, stats, merge). | M2 |
| M4 | UI Foundation | Design tokens, themes, router shell, layout + nav rail, i18n (en/ar, RTL), command palette skeleton. | M1–M3 |
| M5 | Library MVP | Manual add, media CRUD, library grid/list/compact, filters + sort, media detail page, trash/undo. | M2–M4 |
| M6 | Tracking | Node trees, per-node progress (incl. novel chapter read-state), Normal/Manual mode, quick capture, status transitions, dashboard widgets. | M5 |
| M7 | Providers | Coordinator + AniList/TMDB/MangaDex/OpenLibrary (+ Jikan/Google fallback); external search; enrich; identity/dedup; optional Hardcover/Bangumi. | M3, M5 |
| M8 | Import/Export | Import pipeline + preview + reports; JSON/CSV/Markdown export; Goodreads/StoryGraph CSV + AniList/MAL imports. | M7 |
| M9 | Reviews & Collections | Reviews/notes/tags, favorites, collections + smart lists, bulk operations, content warnings. | M5 |
| M10 | Stats & Calendar | Stats service + UI (incl. reading recap), calendar, activity log polish. | M6 |
| M11 | Backup & Recovery | Backup/restore/rotation/validation, auto-backup, merge with conflict preview. | M2, M3, M6 |
| M12 | UX Polish | Shortcuts complete, command palette full, states audit, a11y pass, RTL pass, performance pass. | M5–M11 |
| M13 | Testing & Release | Integration/E2E suites, benchmarks, packaging, Alpha → Beta → Stable. | all |
| FX | Future Scope | Post-Stable, behind designed seams: cloud sync, plugins, AI (opt-in), mobile, more importers/content types. | M13 |

**Dependency spine:** M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8 → M9 → M10 → M11 → M12 → M13.
**Parallel tracks after M3:** (M4 UI shell) ‖ (M5 library) ‖ (M7 provider work once M2+M3 land).

---

## 3. Mission list (everything)

Legend — **Pri:** Core (must ship) · Important (should ship) · Optional (if time, no scope creep).
**Cplx:** S (≤1 day) · M (2–5 days) · L (1–2 weeks). States:
`BACKLOG · READY · IN_PROGRESS · REVIEW · TESTING · DONE · BLOCKED · CANCELLED`.

**Mission log** (most recent first):

| Mission | Status |
|---------|--------|
| MISSION-042 Media detail page: hero, meta tabs, actions (overview/detail/tracking/review shell) | **DONE** (2026-08-12) — `MediaDetailPage` (/library/:id): hero with `Link` back-to-library, poster placeholder (2:3, type icon), title + original title, content-type/status badges + release year, synopsis ✓ · detail navigation: framework-free `Tabs` primitive (Radix-free, `useId` wiring, `role=tablist` + `aria-selected`/`aria-controls`/`aria-labelledby`, full keyboard nav ←/→ wrap + Home/End focus-move without changing selection, only the active `role=tabpanel` renders) ✓ · tabbed sections: **Overview** renders the full aggregate facts (format, status, year, language, country, pages/episodes/chapters/duration, genres as badges) via `MetaCell`; **Details / Tracking / Review** are placeholders wired to MISSION-046 / MISSION-073 ✓ · loading skeleton (hero + tab strip + body), error state with Retry (`refetch`), not-found state for unknown ids ✓ · data layer: `media_get` IPC command (Rust service `get_media` + repo `media::get` returns the full `MediaRecord` with alt titles/people/genres/tags/external ids/relations; `to_record` maps the domain aggregate; `MediaRecord`/`AltTitle`/`ExternalId`/`MediaRelation` now `Serialize`) ✓ · `useMediaDetailQuery(id)` + `queryKeys.media.detail(id)` re-keyed from number to string ✓ · grid/compact cards + list rows are now `<Link>`s to the detail page (aria-label = title; library tests query cards by role link) ✓ · route table (MISSION-032) gains `library/:id` ✓ · i18n `detail.*` en/ar keys with parity kept ✓ · tests: `MediaDetailPage.test.tsx` (hero/badges/back-link/invoke args, overview facts + genre badges, tab keyboard switch + shell placeholders, error→retry, not-found — 5) + `Tabs` primitive tests (semantics, click select, arrow-key nav — 3); server tests `get_media_returns_full_aggregate` + `get_media_returns_none_for_unknown_id`; 113 TS + 170 Rust tests, tsc ✓, eslint 0 errors ✓, prettier ✓, codegen check ✓, cargo fmt ✓, clippy ✓ |
| MISSION-041 Library filter panel + sort menu + group-by + facets endpoint | **DONE** (2026-08-12) — filter panel: `LibraryFilterBar` (type/format/pub-status/genre/tag/year/favorite) + sort menu (title/last-updated/release-year, asc+desc) + group-by (none/content-type/pub-status/year) ✓ · facets stay true to data: new `media_facets` command returns distinct `formats`/`genres`/`tags`/`years` present in the library (repo `facets()`, genre/tag rows carry id+name) so options never drift from rows ✓ · `media_list` gains `format` + `year` filters (IPC contract + codegen regenerated) ✓ · `filters.ts`: `LibraryFilters` → typed `MediaListArgs` (`filtersToArgs`), `activeFilterCount`; `useMediaListQuery` now keys on args so filtering re-queries ✓ · `grouping.ts`: `buildLibraryRows` turns the sorted list into virtualized rows interleaving group-header rows with item rows — enum-ordered groups (content type 10-kind schema order, pub status 6-kind), year descending, `null` release year bucketed last as "Unknown"; `VirtualizedLibrary` virtualizes headers + items at 36px vs density row sizes ✓ · no-results state with clear-filters when a filter matches nothing (library empty + no filters still shows the add-first EmptyLibrary) ✓ · tests rewritten around route-typed `mockLibrary` (11, incl. filter/sort rewriting `media_list` args) + 6 `grouping.test.ts` unit tests; 105 TS, 168 Rust, tsc ✓, eslint 0 errors ✓, prettier ✓, codegen check ✓ |
| M4 · foundation review fixes (type scale, switcher a11y, dead keys, shared labels) | **DONE** (2026-08-12) — post-037 review-fix pass ✓ · Tailwind `@theme` type scale now mirrors `tokens.css` (§3): added the missing `--text-md:1rem` so the TopBar `<h1>` and EmptyState `<h2>` render at 16px (were silently body-sized via the dead `text-md` class) and aligned `text-xs/sm/base` to 12/13/14 (was 13/14/16) ✓ · a11y: added `role="group"` to the TopBar theme switcher and `LanguageSwitcher` wrappers so their `aria-label`s are actually exposed to assistive tech (Settings already had it) ✓ · i18n: removed the dead `page.status_bar` keys (en/ar) — `StatusBar` uses `shell.status.*` ✓ · DRY: extracted `LANGUAGE_SHORT_LABELS` into `i18n/index.ts`; `LanguageSwitcher` + `SettingsPage` now share it instead of two identical per-file maps ✓ · token-scale compliance: Settings sections `rounded-xl p-5` → `rounded-md p-6`, segmented pickers `px-3.5 py-1.5` → `px-3 py-1` (match TopBar), EmptyState `py-16` → `py-12` ✓ · tests: `preferences.test.tsx` settings groups now scoped to `within(main)` (TopBar + Settings both expose Theme/Language groups) ✓ · 92/92 TS tests, tsc ✓, eslint 0 errors ✓, prettier ✓ |
| MISSION-037 A11y baseline: focus ring, reduced-motion, semantic shell, screen-reader labels | **DONE** (2026-08-11) — `SkipLink` (first tab stop) slides into view on `:focus-visible` (new `.skip-link` styles on tokens; slides via transform + ease-out, safe under reduced-motion) and moves real focus to `main#main-content` (`tabIndex=-1`, `outline-none`) on activation ✓ · semantic shell audited: nav landmark gets a proper translated `a11y.navTitle` label (was the page label), `banner`/`main`/`contentinfo` landmarks verified, headings per page ✓ · screen-reader pass: TopBar theme group `a11y.theme`, LanguageSwitcher `a11y.language` (was hardcoded English), Dialog close button gains `closeLabel` prop (translated `a11y.close`, passed by the palette) ✓ · new `a11y.*` en/ar keys (parity kept) ✓ · tests (`components/shell/a11y.test.tsx`): landmark roles + main id, skip link is first tab stop + moves focus to main on Enter, control labels (Light/EN/ع, Language/Theme groups), palette combobox + Close names, Arabic landmark translation; focus ring + reduced-motion were already in `global.css` (MISSION-030) ✓ · 73 TS tests, lint 0 errors, build, prettier, codegen check ✓ |
| MISSION-036 Command palette skeleton (Ctrl/Cmd+K) + shortcut registry | **DONE** (2026-08-11) — `shortcuts/keys.ts`: framework-free combo primitives — platform detection, `parseKeyCombo` ("Mod+K" = Cmd on macOS / Ctrl elsewhere), strict `matchesKeyCombo` against KeyboardEvent modifiers, `formatKeyCombo` hints (⌘K / Ctrl+K, Esc labels) ✓ · `shortcuts/useShortcuts.ts`: global window keydown registry with `enabled` gates and editable-target guard (inputs/textareas/contenteditable swallow bare-letter combos unless a modifier is held) ✓ · `features/command-palette/commands.tsx`: typed `PaletteCommand` registry + `buildPaletteCommands` (nav to every section, theme light/dark/system) + substring `filterPaletteCommands` (label + keywords) ✓ · `CommandPalette.tsx`: Radix Dialog overlay, Combobox input with ↑/↓ wrap nav + Enter + Escape, mouse-hover sync, grouped options (Navigation/Actions) with scroll-to-active, `Mod+K` toggle, Ctrl+K kbd hint, i18n `palette.*` keys en/ar (parity kept) ✓ · theme actions write through `usePreferences().setTheme` (single persistence path); `DialogContent` gained a `noPadding` option for the flush layout ✓ · mounted in AppShell; app test harness now includes `PreferencesProvider` ✓ · tests: combo parse/match/format matrix, palette opens/filters/navigates/runs-theme/navigates-to-settings; 66 TS tests, lint 0 errors, build, prettier, codegen check ✓ |
| MISSION-035 TanStack Query client + typed command wrappers (`api.ts`) + query keys | **DONE** (2026-08-11) — `@tanstack/react-query` v5 installed; `api/queryClient.ts`: `createQueryClient()` factory with local-first defaults (`retry:false`, `staleTime:60s`, `gcTime:5m`, `refetchOnWindowFocus:false`) + app singleton ✓ · `api/queryKeys.ts`: typed readonly-tuple key factory with scope fan-out (`all`/`lists`/`list(filters)`/`detail(id)` dashboards) for media, tracking, review, collection, stats, search, settings, task + system (greet) ✓ · `api/api.ts`: typed command wrapper object (`api`) + React Query hooks (`useGreetQuery` caching under `system.greeting(name)`, `useGreetMutation` writing its result into the cache) — features never touch `invoke` or raw keys ✓ · `api/index.ts` re-exports the whole boundary; `QueryClientProvider` mounted in main.tsx (contexts compose: Query → Theme → Preferences → Toasts) ✓ · tests: client defaults, key factory shapes/fan-out, wrapper hooks resolve+cache and mutation seeds cache via mocked `invoke`; 55 TS tests, lint 0 errors, build, prettier, codegen check ✓ |
| MISSION-034 Settings store: preferences model + persistence | **DONE** (2026-08-11) — `tauri-plugin-store` 2.4 wired: Rust dep, `tauri_plugin_store::Builder` registered, `store:default` capability added ✓ · `preferences/types.ts` (`Preferences { theme, language }` + defaults) ✓ · `preferences/repository.ts`: single `PreferencesRepository` with runtime backend selection — Tauri plugin `Store` (`settings.json`) in the desktop shell, localStorage (`mylore.preferences`) elsewhere; tolerant `parsePreferences` (corrupt/partial storage → defaults); cached repo ✓ · `PreferencesProvider`: renders with boot values (no flash), reconciles with the persisted store on load, and every change persists + mirrors the boot-cache keys (`mylore.theme`/`mylore.lang`) so `initTheme`/`initI18n` stay in sync; `usePreferences()` hook (context split into `PreferencesContext.ts`/`usePreferences.ts` per `ThemeContext`/`useTheme` convention) ✓ · real Settings page replaces the placeholder — theme (light/dark/system) + language (EN/ع) segmented controls (`role="group"` aria-labelled), i18n keys added (en/ar parity kept) ✓ · provider mounted in main.tsx ✓ · TS tests: repo round-trip/null/corrupt, settings persistence (theme→`data-theme` + store, language→rtl/lang + store), apply-on-mount; 50 TS + 165 Rust tests, lint 0 errors, build, prettier, cargo check/test ✓ |
| MISSION-033 i18n: i18next en/ar, ICU, RTL wiring (`dir`, logical props), locale switcher | **DONE** (2026-08-11) — i18next + react-i18next (bundler, `useSuspense: false`) ✓ · `i18n/locales.ts` en/ar resource trees (nav, shell/status, theme + full page hints) with ICU plural categories via Intl (`counts_one/other` en · `zero/one/two/few/many/other` ar); parity is enforced by a locale test (exact match for non-plural keys, Arabic superset for plural forms, non-empty values) ✓ · `i18n/index.ts`: bootstrap with `LOCALE_STORAGE_KEY` persistence, browser-language fallback, `applyLanguage` sets `lang` + `dir` (ar→rtl) on `<html>`, `setLanguage` persists + re-renders, `initI18n()` pre-paint in main.tsx (no FOUC), `useLanguage` hook ✓ · RTL made logical: NavRail `border-e`, Toast viewport `end-0`, Dialog close `end-4` (flex layout already mirrors) ✓ · `LanguageSwitcher` (EN/ع segmented control) in the TopBar next to the theme switcher; whole shell translated (TopBar title, NavRail labels, StatusBar counts + version, empty-state titles/hints) ✓ · 43 tests, lint 0 problems, tsc+vite build, prettier, codegen check ✓ |
| MISSION-032 Router + app shell: nav rail, topbar, status bar; empty-state pages | **DONE** (2026-08-11) — React Router (hash router for the Tauri webview scheme; `createHashRouter`, no server rewrites on reload) ✓ · `navigation.ts` is the single source of path/label/icon for rail + top bar + routes (8 sections: Library, Search, Discover, Collections, Reviews, Stats, Calendar, Settings) ✓ · `components/shell/`: **AppShell** (nav rail + top bar + status bar around `<Outlet>`, h-screen flex, mirrors in RTL), **NavRail** (icon+text NavLinks, active = accent-soft, `end` for the index section), **TopBar** (page title from route + app-wide theme switcher; `THEME_CHOICES` moved to `themes/preferences.ts`), **StatusBar** (v0.1.0 / "0 titles" placeholder) ✓ · `EmptyState` primitive (icon + title + hint + action) consumed by 8 placeholder feature pages (`features/index.tsx`) ✓ · `routes.tsx` route table shared with tests via `createMemoryRouter`; root redirects to Library ✓ · primitives showcase App removed; `main.tsx` renders ThemeProvider → ToastProvider → RouterProvider ✓ · 32 tests (shell: redirect, nav navigation, active highlight, status bar, theme switch), lint 0 problems, tsc+vite build, prettier, codegen check ✓ |
| MISSION-031 Tailwind + design-system primitives (Button, Input, Dialog, Popover, Toast, Skeleton…) on Radix | **DONE** (2026-08-11) — Tailwind v4 via `@tailwindcss/vite` + `@theme` mapping the MISSION-030 tokens onto utilities (`bg-bg-surface`, `text-text-primary`, `rounded-md`, `shadow-sm`, status palette; aliased `--elevation-*` so Tailwind's shadow keys dereference without self-collision) ✓ · primitives in `src/components/ui/` (barrel `index.ts`, `cn()` helper): **Button** (primary/secondary/ghost/danger, sm/md, `asChild` via `@radix-ui/react-slot`, default `type=button`) ✓ · **InputField + TextareaField** (Radix Label, required label, inline error slot with `role=alert` + `aria-invalid`) ✓ · **Badge** (status palette + neutral) ✓ · **Dialog/Popover** (Radix; focus trap, Esc, dir-aware, overlay + raised surface + elevation) ✓ · **Toast** (Provider+useToast: success/error/info, undo action, auto-dismiss, swipe) ✓ · **Skeleton** (pulse, reduced-motion-safe) ✓ · App switch to a primitives showcase (themes switcher retained); `lucide-react` icons added ✓ · test setup gains a `ResizeObserver` polyfill (jsdom/Radix) ✓ · 32 tests (20→32), lint 0 problems (eslint override scoping fast-refresh off for ui modules), tsc+vite build, prettier, codegen check ✓ |
| MISSION-030 Design tokens (colors/type/spacing/radius/elevation), light+dark themes, `data-theme` | **DONE** (2026-08-11) — `src/design-tokens/tokens.css` as the single source of truth (DESIGN_SYSTEM.md): full palette both themes on `:root`/`:root[data-theme=dark]` (bg-base/-surface/-raised/-hover, border-subtle/-strong, text-primary/-secondary/-tertiary, accent/-hover/-soft, ok, warn, danger, info) + `color-scheme` ✓ · tokenized status→color mapping (`status-planned/-inprogress/-completed/-onhold/-dropped/-repeat`, AA pairs) ✓ · type scale (12–36rem) + tabular-nums, spacing 4px unit (4–48), radius 6/10/16/999, elevation shadow-sm/lg, focus ring 2px accent w/ 3px gap, motion 120–200ms ease-out, control heights ✓ · `src/design-tokens/index.ts` mirrors values for JS/TS (charts, inline styles) ✓ · `themes/theme.ts` framework-free controller: `ThemePreference light|dark|system` (system default), `matchSystemTheme()` (prefers-color-scheme), `applyTheme`/`createThemeSystem` setting `data-theme` on `<html>`, `localStorage` persistence (`mylore.theme`), graceful fallbacks when storage/matchMedia unavailable ✓ · `ThemeProvider` + `ThemeContext` + `useTheme`; system-preference live-follow (matchMedia change listener, derived-state render so no state-in-effect churn) ✓ · `initTheme()` runs in `main.tsx` pre-render (no FOUC) ✓ · App scaffold converted to token-driven classes + working light/dark/system switcher ✓ · global.css: base styles, AA focus ring, reduced-motion, selection ✓ · 12 (+3 updated App/theme) tests, lint 0 problems, tsc+vite build, prettier, codegen check ✓ |
| MISSION-025 Title normalization (case/unicode/diacritic fold, script-aware) + title matching | **DONE** (2026-08-11) — `domain/normalize` (pure, no I/O) ✓ · `fold_title`: NFC compose → full Unicode lowercase → width normalize (fullwidth ASCII → halfwidth, fullwidth space, halfwidth katakana → fullwidth via table) → NFD + drop combining marks (**except kana voicing U+3099/U+309A**: パン stays distinct from ハン) → Arabic consonant variants (أ/إ/آ/ٱ → ا, ى → ي, ة → ه) + × → x → script-aware filter (Han/kana/hangul: drop spaces+punctuation; spaced scripts: collapse whitespace + separators -,_,/,·,–,— to one space, drop other punctuation) ✓ · `title_matches` (fold equality), `title_contains` (fold substring) for the identity stage ✓ · `unicode-normalization` 0.1.25 added as a direct dependency (already vendored transitively) ✓ · 12 new unit tests (129/129 total) ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-026 IdentityService: exact (provider, ext_id) + fuzzy scoring + candidate ranking | **DONE** (2026-08-11) — `domain/identity` (pure, no I/O) ✓ · `IdentityCandidate { media_id, titles, external_ids }` + `IdentityKind` (Exact / TitleExact / Fuzzy / None) ✓ · `exact_external_id`: same (provider, value) on file → definitive (score 1.0) ✓ · `titles_exact` (fold-equal any title incl. original/alternatives → 0.95, TITLE_EXACT_SCORE) ✓ · `title_similarity`: token Jaccard + containment + bigram Jaccard on MISSION-025 folds, bigram only refines pairs with token/containment overlap so stray bigrams ("on" in One Piece/Attack on Titan) give no false positives ✓ · `best_title_similarity` across all title pairs ✓ · `score_candidate` + `rank_candidates` (score desc, media_id tie-break for determinism) + `best_match` (skips kind None) ✓ · 9 new unit tests (138/138 total) ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-027 StatsService: pure computations (counts, hours, completion, avg rating, distributions) | **DONE** (2026-08-11) — `domain/stats` (pure, no I/O) ✓ · `MediaStatsRow { media_id, content_type, core_status, rating, favorite, release_year, progress }` projection + `compute_stats(&[MediaStatsRow]) -> StatsSummary` ✓ · counts per status and per content type (schema ALL order) ✓ · rating distribution 1..=10 + `avg_rating` (mean of non-null) ✓ · completion: `completed_media`, `completion_rate` (completed/total, None when empty), `avg_percent` (mean of aggregate percents) ✓ · time from real data only: `consumed_minutes` (sum of node-level completed minutes) + `consumed_hours()`; reading reported as `consumed_pages` (Pages-weight aggregates) — no invented pages→time conversion (product decision) ✓ · `year_counts` distribution (ascending) ✓ · `favorites` ✓ · 7 new unit tests (145/145 total) ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-028 MergeService: merge plan, conflict report, re-parenting, before-image | **DONE** (2026-08-11) — `domain/merge` (pure, no I/O) ✓ · `plan_merge(survivor, duplicate, duplicate_nodes, duplicate_collection_ids, survivor_has_review, survivor_has_tracking) -> MergePlan` ✓ · merged metadata: survivor identity kept, scalars prefer survivor and fall back to duplicate, sets (titles/genres/tags/people/relations/external_ids) unioned + deduped; duplicate main title becomes an alternative when it differs; duplicate original title is a fallback ✓ · `conflicts: Vec<FieldConflict>` only for *different non-empty* values (scalars, content_type, title via fold, external_id provider collisions) ✓ · re-parenting: duplicate nodes re-keyed to survivor id, parent links and node ids preserved ✓ · moves: review/tracking only when survivor lacks one; collection memberships always re-keyed ✓ · `BeforeImage { survivor, duplicate }` for undo ✓ · self-relations to the survivor dropped, external ids stay provider-unique, merged record re-validated ✓ · 13 new unit tests (158/158 total) ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-029 Service unit tests: progress math, status, dedup, stats, merge | **DONE** (2026-08-11) — `tests/domain_services.rs` integration crate over `mylore_lib::domain::*` (pure, in-memory) ✓ · tracking lifecycle: aggregate → `suggest_auto_status` (in_progress → completed → repeat) with `apply_transition` stamping started_at/finished_at, repeat_count increment + finished_at clear, reversible regression ✓ · stats combining watch time/reading/ratings across content types (`consumed_minutes` from durations-only, pages by book weight, avg percent = mean of integer percents) ✓ · dedup → merge → re-parenting: exact external-id match on the family, duplicate collection memberships moved, genre/title sets unioned, duplicate nodes re-keyed to survivor, before-image present ✓ · title-variant dedup via fold (`タイトル`, case, ×/x) → TitleExact, merged main survives, no alternative duplication ✓ · Arabic variant dedup (anime title vs Arabic alt) + external-id union ✓ · merged media's node set aggregates to the same progress as the original ✓ · custom-status bucket sets status count, Repeat-guard rejects entering from Planned ✓ · 7 new integration tests (158 + 7 = 165/165 total) ✓ · fmt ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-024 Status engine: core statuses, custom statuses, auto-transition rules (reversible) | **DONE** (2026-08-11) — `domain/status` (pure, no I/O) ✓ · `CoreStatus` classification: `is_terminal` (completed/dropped), `is_active` (in_progress/repeat), `is_not_started` (planned/wishlist) ✓ · explicit transition matrix (`can_transition`): permissive like MAL/AniList with one rule — **Repeat requires prior consumption** (in_progress/completed/dropped/repeat only) ✓ · `apply_transition(tracking, to, today)` (clock-free) stamps started_at (active) / finished_at (terminal, only when absent), clears finished_at when leaving terminal (reversible), increments repeat_count on entering Repeat and resets on exit; result re-validated so it can never be invalid ✓ · `CustomStatus { id, name, bucket, sort_order }` + `effective_status` (custom bucket overrides core status) ✓ · `suggest_auto_status(&ProgressAggregate)` auto-transition rule: all nodes consumed → completed; some → in_progress; none → planned; **None when no node data**; explicit + reversible (un-marking moves completed → in_progress → planned) ✓ · 11 new unit tests (117/117 total) ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-023 Progress engine: per-contentType templates, aggregates (pages/chapters/episodes); unit tests | **DONE** (2026-08-11) — `domain/progress` (pure, no I/O) ✓ · `ProgressTemplate::for_content_type` covers all 10 content types: anime/tv/movie → Episode/Count/Watched; manga/manhwa/manhua/novel/web_novel → Chapter/Count/Read; book → Chapter/**Pages**/Read; other → Node/Count/Read ✓ · `unit_label()` for UI ("episodes"/"chapters"/"pages") ✓ · `aggregate()` folds `NodeTick`s: only template-unit nodes count, weight = 1 or `page_count.unwrap_or(1)`, only consuming state counts, `percent` = `saturating_mul(100)/total` (None when tree empty), minutes summed only when a node carries duration ✓ · `estimated_total_units(content_type, &MediaRuntime)` → pages/ep_count/ch_count (book uses pages, not chapter count) ✓ · `with_estimate` fills totals only when the node tree contributed 0 ✓ · 10 new unit tests (106/106 total) ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-022 Domain types: `Media`, `ContentNode`, `Tracking`, `Review`, value objects; invariant guards | **DONE** (2026-08-11) — `domain/{enums,value_objects,media,content_node,tracking,review,error}` (pure, no I/O) ✓ · `string_enum!` macro mirrors every SQL CHECK (ContentType, MediaStatus, NodeKind, NodeProgressState, CoreStatus, PersonRole, MediaRelationKind) with `as_str`/`FromStr` — enum-valid ⇒ CHECK-valid ✓ · value objects: `MediaId`, `Rating(1..10)`, `DateOnly(YYYY-MM-DD)`, `LanguageCode`, `ProviderId`, `Title` (≥1 title, dedup incl. original), `ExternalId` ✓ · invariant guards: media (unique provider ext-ids, no self-relation, start≤end, timestamps), ContentNode (1-based position), NodeProgress (read/watched ⇒ read_at; unread/skipped ⇒ no read_at), Tracking (repeat_count only on Repeat, finished ⇒ terminal status, finish≥start), Review (timestamp order, spoiler requires text) ✓ · `DomainError` + `From<DomainError> for AppError` ✓ · 26 new unit tests (96/96 total) ✓ · clippy -D warnings ✓ |
| MISSION-021 Benchmarks: insert 1k/10k, search 10k/50k/100k rows, bulk import timing | **DONE** (2026-08-11) — `src-tauri/benches/database.rs` (criterion, release) ✓ · fresh in-memory migrated DB per insert sample (results independent of prior samples) ✓ · insert repo path (`media::create` per-row tx) 1k ≈ 349 ms (~0.35 ms/row), 10k ≈ 3.66 s ✓ · insert bulk raw one-tx 1k ≈ 184 ms, 10k ≈ 1.90 s (~2× faster) ✓ · search FTS5 **flat at ~110 µs** across 10k/50k/100k rows (selective ~1% hit + no-match) — index-bound, not data-bound ✓ · bulk import: raw one-tx 10k ≈ 1.90 s vs repo loop ≈ 3.76 s → import pipeline (MISSION-066) must batch in one tx ✓ · DATABASE.md §5.2 ✓ · clippy -D warnings ✓ · 70/70 tests ✓ |
| MISSION-020 DB integration tests: CRUD, FKs, cascade, FTS query, transaction rollback | **DONE** (2026-08-11) — `infrastructure::integration_tests` (cfg(test)) over the fully-migrated schema ✓ · full lifecycle: create media + nodes/tracking/progress/review/collection/activity → delete media → every aggregate cascades, FTS empties, unrelated media + collection + person survive ✓ · manual tx: failed statement → whole tx rolls back (incl. FTS), commit persists ✓ · repo-internal atomicity: failing link rolls back `media::create` AND `media::update` (previous aggregate untouched) ✓ · FTS follows review body edits (index add + clear + delete) ✓ · FK rejection at repo boundary for every aggregate ✓ · 70/70 tests ✓ · clippy -D warnings ✓ · ROADMAP ✓ |
| MISSION-019 Repositories: media, node, tracking, review, collection, asset, activity (sqlx typed) | **DONE** (2026-08-11) — `infrastructure::repositories/{media,node,tracking,review,collection,asset,activity}` ✓ · media: full-aggregate create/get/update (row + links in one tx), delete, `find_by_external_id`, `list` (content_type/pub_status/genre/tag/favorite filters, sort, pagination) + `count`, FTS `search` (unicode61 + trigram, `fts::normalize_query` Arabic fold) ✓ · node: create/reparent/update/delete/children with `content_node` validators (cross-media + cycle rejection) ✓ · tracking: row upsert + `node_progress` upsert/read + `count_nodes_in_state` ✓ · review/collection/asset/activity CRUD ✓ · repos are clock-free, never write FTS tables ✓ · 64/64 tests ✓ · clippy -D warnings ✓ · DATABASE.md §5.1 ✓ |
| MISSION-018 FTS5 `media_fts` + triggers + rebuild; multilingual tokenization (unicode61 + trigram for CJK) | **DONE** (2026-08-11) — `migrations/0007_media_fts.sql` ✓ · `v_media_fts_source` view with Arabic normalization (Alef variants→ا, ى→ي, ة→ه, diacritics/tanween stripped) ✓ · `media_fts` (unicode61, 9 columns) + `media_fts_cjk` (trigram) — **stored content**, not contentless (FTS5 contentless `'delete'` either needs deleted values or silently orphans terms) ✓ · 21 refresh triggers (ins/del/upd on media + 6 feeder tables) + backfill ✓ · `PRAGMA recursive_triggers = ON` in `db::connect` + `test_support::in_memory_pool` so cascades re-fire FTS triggers ✓ · `infrastructure::fts::rebuild` (transactional wipe + repopulate) ✓ · 45/45 tests (schema/triggers, both indexes, CJK trigram substring, Arabic fold, alt-title refresh, cascade refresh, update refresh, rebuild) ✓ · clippy -D warnings ✓ |
| MISSION-017 `review`, `collection`, `collection_member`, `asset`, `activity`, `trash`, `settings` | **DONE** (2026-08-11) — `migrations/0006_user_aggregates.sql` ✓ · media cover/banner asset columns added via ALTER TABLE ADD COLUMN (deferred FK from 0002) ✓ · CHECKs: rating 1..10, favorites/spoiler/smart/restored booleans, asset kind/status, activity kind, trash kind ✓ · `idx_activity_media` ✓ · `provider_setting` deferred to MISSION-063 ✓ · 36/36 tests (asset SET NULL, review/collection/activity cascades, checks, settings roundtrip) ✓ · clippy -D warnings ✓ |
| MISSION-016 `tracking`, `status` (seeded core), `node_progress` + CHECKs | **DONE** (2026-08-11) — `migrations/0005_tracking.sql` ✓ · 7 core statuses seeded (is_system=1) ✓ · CHECKs: core_status, bucket, repeat_count>=0, node state, rating 1..10 ✓ · `idx_tracking_core_status`/`idx_tracking_updated_at` ✓ · 30/30 tests (seeds, checks, FK cascade, custom-status SET NULL) ✓ · clippy -D warnings ✓ |
| MISSION-015 `media_external_id`, `media_relation` + unique constraints | **DONE** (2026-08-11) — `migrations/0004_media_identity.sql` ✓ · PK(provider, ext_id) + UNIQUE(media_id, provider) exact-identity rules ✓ · relation CHECK + CHECK(from_id <> to_id) ✓ · 25/25 tests (uniques, cascades, self/missing/bad-relation rejection) ✓ · clippy -D warnings ✓ |
| MISSION-014 `content_node` tree + indexes; FK validation helpers | **DONE** (2026-08-11) — `migrations/0003_content_node.sql` (kind CHECK, `idx_node_media`/`idx_node_parent`) ✓ · `infrastructure::content_node` validators: parent-belongs-to-media + acyclic (SQLite can't express cross-row tree invariants) ✓ · `AppError::Validation` ✓ · shared test helpers moved to `test_support` ✓ · 21/21 tests (schema/indexes, media-cascade, kind CHECK, cross-media + cycle rejection) ✓ · clippy -D warnings ✓ |
| MISSION-013 `media`, `media_alt_title`, `person`/`media_person`, `genre`, `tag` (+ joins) + seeds | **DONE** (2026-08-11) — `migrations/0002_media.sql` ✓ · enum CHECKs (content_type, pub_status, role, scope) ✓ · seeds: 18 genres + 21 domain tags ✓ · asset FK columns deferred to MISSION-017 (SQLite rejects writes through an FK to a missing parent) ✓ · 16/16 tests (schema+seeds, media/person cascade, FK enforced) ✓ · clippy -D warnings ✓ |
| MISSION-012 Migration runner (`sqlx::migrate!`), versioned `migrations/`, transaction-wrapped | **DONE** (2026-08-11) — `db::migrate` wired into `init` ✓ · `migrations/0001_init.sql` (`app_meta`) ✓ · per-migration transaction verified in sqlx-sqlite 0.8.6 ✓ · `AppError::Migration` ✓ · idempotency + corrupt-db tests (Windows WAL/SHM lock fix) ✓ · clippy -D warnings ✓ |
| MISSION-011 sqlx pool + PRAGMAs + startup integrity check | **DONE** (2026-08-11) — `db::init` (FK/WAL/busy_timeout=5s/synchronous=Normal, max 5 conns) ✓ · `PRAGMA integrity_check` at startup ✓ · pool as managed state (`app.manage`) ✓ · `AppError::Database` ✓ · tests ✓ · clippy -D warnings ✓ |
| MISSION-010 Window shell: titlebar, min sizes, app id, CSP | **REVIEW** — productName/identifier `MyLore`/`com.mylore.app` ✓ · window 1100x760 min 900x600 centered ✓ · strict prod CSP (no unsafe-inline) + dev CSP ✓ · config guard test ✓ · cargo build validates ✓ · 7/7 tests ✓ |
| MISSION-009 Typed IPC boundary + codegen | **REVIEW** — contract in `scripts/ipc-contract.json` ✓ · `npm run codegen` → `src/api/ipc.generated.ts` ✓ · drift-guard (Rust #[command] scan + --check) ✓ · App.tsx uses typed `greet` ✓ · 4/4 tests ✓ · lint/build ✓ |
| MISSION-008 Logging (tracing → rolling file, no secrets) + `AppError` | **REVIEW** — clippy -D warnings ✓ · 6/6 tests ✓ · cargo build ✓ · rolling daily logs + stdout ✓ · AppError→frontend as string ✓ |
| MISSION-007 GitHub Actions CI (win/ubuntu/macos) | **REVIEW** — actionlint ✓ · prettier ✓ · all steps pass locally (fmt/clippy/test/build/lint) |
| MISSION-006 Rust unit-test harness + in-memory sqlite helper | **DONE** (2026-08-11) — `cargo test` 2/2 ✓ · clippy -D warnings ✓ · fmt ✓ |
| MISSION-005 Vitest + Testing Library harness + sample test | **DONE** (2026-08-11) — 2 tests pass · lint ✓ · format ✓ · build ✓ |
| MISSION-004 Rust rustfmt + Clippy + crate layout | **DONE** (2026-08-11) — fmt ✓ · clippy -D warnings ✓ · layered crate layout per PROJECT_MAP |
| MISSION-003 ESLint + Prettier + pre-commit hook | **DONE** (2026-08-11) — eslint ✓ · prettier ✓ · husky+lint-staged ✓ · .gitattributes ✓ |
| MISSION-002 TS strict + path aliases + editor config | **DONE** (2026-08-11) — strict flags ✓ · `@/` alias (tsc+vite) ✓ · .editorconfig ✓ |
| MISSION-001 Scaffold Tauri 2 + React + TS | **DONE** (2026-08-11) — npm build ✓ · cargo build ✓ · app launched & user-tested ✓ |

### M1 · Foundation (MISSION-001 … 010)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-001 | Scaffold Tauri 2 + React + TS (Vite); baseline config (`tauri.conf.json`, capabilities). | — | Core | M |
| MISSION-002 | TypeScript strict config, path aliases, editor config. | 001 | Core | S |
| MISSION-003 | ESLint + Prettier (+ pre-commit hook via lint-staged). | 002 | Core | S |
| MISSION-004 | Rust: rustfmt + Clippy (deny warnings in CI), crate layout. | 001 | Core | S |
| MISSION-005 | Vitest + @testing-library/react harness; sample test. | 002 | Core | S |
| MISSION-006 | Rust unit-test harness (`cargo test`) with in-memory sqlite helper. | 004 | Core | S |
| MISSION-007 | GitHub Actions: build + lint + test (windows/ubuntu/macos-latest). | 003..006 | Core | M |
| MISSION-008 | Logging (tracing → rolling file, no secrets) + `AppError` skeleton. | 001 | Core | M |
| MISSION-009 | Typed IPC boundary: shared TS types for commands/events + codegen. | 002,008 | Core | M |
| MISSION-010 | Window shell: titlebar, min sizes, app identifier, CSP in config. | 001 | Core | S |

### M2 · Database (MISSION-011 … 021)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-011 | sqlx pool + PRAGMAs (FK, WAL, busy_timeout) as managed state; startup integrity check. | 008 | Core | M |
| MISSION-012 | Migration runner (`sqlx::migrate!`), versioned `migrations/`, transaction-wrapped. | 011 | Core | S |
| MISSION-013 | `media`, `media_alt_title`, `person`/`media_person`, `genre`, `tag` tables + seeds. | 012 | Core | M |
| MISSION-014 | `content_node` tree + indexes; FK validation helpers. | 013 | Core | M |
| MISSION-015 | `media_external_id`, `media_relation` + unique constraints. | 014 | Core | M |
| MISSION-016 | `tracking`, `status` (seeded core), `node_progress` + CHECKs. | 013,014 | Core | M |
| MISSION-017 | `review`, `collection`, `collection_member`, `asset`, `activity`, `trash`, `settings`. | 013..016 | Core | M |
| MISSION-018 | FTS5 `media_fts` + triggers + rebuild; multilingual tokenization (unicode61 + trigram for CJK). | 013,015,016 | Core | L |
| MISSION-019 | Repositories: media, node, tracking, review, collection, asset, activity (sqlx typed). | 011..018 | Core | L |
| MISSION-020 | DB integration tests (CRUD, FKs, cascade, FTS query, transaction rollback). | 019 | Core | M |
| MISSION-021 | Benchmarks: insert 1k/10k, search 10k/50k/100k rows, bulk import timing. | 020 | Important | M |

### M3 · Domain Layer (MISSION-022 … 029)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-022 | Domain types: `Media`, `ContentNode`, `Tracking`, `Review`, value objects; invariant guards. | 012 | Core | M |
| MISSION-023 | Progress engine: per-contentType templates, aggregates (pages/chapters/episodes); unit tests. | 022 | Core | M |
| MISSION-024 | Status engine: core statuses, custom statuses, auto-transition rules (reversible). | 023 | Core | M |
| MISSION-025 | Title normalization (case/unicode/diacritic fold, script-aware) + title matching. | 023 | Core | M |
| MISSION-026 | IdentityService: exact (provider, ext_id) + fuzzy scoring + candidate ranking. | 025 | Core | M |
| MISSION-027 | StatsService: pure computations (counts, hours, completion, avg rating, distributions). | 022 | Core | M |
| MISSION-028 | MergeService: merge plan, conflict report, re-parenting, before-image. | 022,026 | Important | L |
| MISSION-029 | Service unit tests: progress math, status, dedup, stats, merge. | 023..028 | Core | M |

### M4 · UI Foundation (MISSION-030 … 037)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-030 Design tokens (colors/type/spacing/radius/elevation), light+dark themes, `data-theme` | **DONE** — see log | 010 | Core | M |
| MISSION-031 Tailwind + design-system primitives (Button, Input, Dialog, Popover, Toast, Skeleton…) on Radix | **DONE** — see log | 030 | Core | L |
| MISSION-032 Router + app shell: nav rail, topbar, status bar; empty-state pages | **DONE** — see log | 031 | Core | M |
| MISSION-033 i18n: i18next en/ar, ICU, RTL wiring (`dir`, logical props), locale switcher | **DONE** — see log | 032 | Core | M |
| MISSION-034 | Settings store (tauri-plugin-store) + preferences model; theme/lang persistence. | 033 | Core | S | **DONE** — see log |
| MISSION-035 | TanStack Query client + typed command wrappers (`api.ts`) + query keys. | 009,032 | Core | M | **DONE** — see log |
| MISSION-036 | Command palette skeleton (Ctrl/Cmd+K) + shortcut registry. | 032 | Important | M | **DONE** — see log |
| MISSION-037 | A11y baseline: focus ring, reduced-motion, semantic shell, screen-reader pass. | 031 | Important | M | **DONE** — see log |

### M5 · Library MVP (MISSION-038 … 045)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-038 | Manual Add dialog (fast entry, validation with Zod) → MediaService command. | 019,022,032 | Core | M |
| MISSION-039 | Library query endpoint (filter/sort/group/paginate) + API. | 019 | Core | M |
| MISSION-040 | Library views: Grid / List / Compact list (virtualized, TanStack Virtual). | 035,039 | Core | L |
| MISSION-041 | Filter panel (type, format, status, genre, tag, year, favorite) + sort menu + group-by. | 039,040 | Core | L | **DONE** — see log |
| MISSION-042 | Media detail page: hero, meta tabs, actions (overview/detail/tracking/review shell). | 035,039 | Core | L | **DONE** — see log |
| MISSION-043 | Local search (FTS) in header + results page (local-first). | 018,039 | Core | M |
| MISSION-044 | Trash/restore UI + undo toast for deletes. | 017,038 | Important | M |
| MISSION-045 | Bulk-select mode + action bar (status, tag, list, delete, export later). | 040,044 | Important | M |

### M6 · Tracking (MISSION-046 … 052)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-046 | Node tree endpoint + UI (seasons→episodes, volumes→chapters; expand/collapse). | 014,019,042 | Core | L |
| MISSION-047 | Per-node progress commands (mark read/watched/skipped, range-mark) + optimistic UI. | 023,046 | Core | M |
| MISSION-048 | Status transitions + auto-complete rule; repeat counter. | 024,047 | Core | M |
| MISSION-049 | Quick capture popover (global hotkey) + in-grid quick controls. | 047,036 | Core | M |
| MISSION-050 | Dashboard widgets: Continue Reading/Watching, Recently Added/Completed, Quick Actions. | 047,048,039 | Core | M |
| MISSION-051 | Activity log writes on all tracking actions. | 017,047 | Core | S |
| MISSION-052 | **Novel/web-novel tracking UX** (research §2b): chapter-list read-state rows + "my status" markers; **Normal (autoTrack) vs Manual mode** per media; **DNF-with-progress** (dropped carries % / chapter). | 047,048 | Important | M |

### M7 · Providers (MISSION-053 … 065)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-053 | Provider trait + capabilities + Coordinator (rate limit, retry/backoff, timeout, cancel, error map). | 022 | Core | L |
| MISSION-054 | AniList adapter (search/details/nodes/relations/external ids; LNs via MANGA/NOVEL) + fixtures. | 053 | Core | L |
| MISSION-055 | TMDB adapter (movies+TV, seasons/episodes, images, attribution) + fixtures. | 053 | Core | L |
| MISSION-056 | MangaDex adapter (manga/manhwa/manhua, chapters, covers) + fixtures. | 053 | Core | L |
| MISSION-057 | OpenLibrary adapter (books, ISBN, covers) + fixtures. | 053 | Core | M |
| MISSION-058 | Fallbacks: Jikan (anime), Google Books (books). | 054,057 | Optional | M |
| MISSION-059 | External search UI (grouped by provider, "in library"/duplicate flags). | 053..057,026 | Core | L |
| MISSION-060 | Import-from-provider flow (search → details → identity check → add). | 059,026 | Core | M |
| MISSION-061 | Enrich metadata (refresh provider fields only; never touch user data) + diff report. | 054..057,042 | Core | M |
| MISSION-062 | Image pipeline: download/cache covers, broken-url handling, cache policy. | 017,053 | Core | M |
| MISSION-063 | Provider settings UI: enable/disable, API keys (keyring), test connection. | 053 | Core | M |
| MISSION-064 | Hardcover adapter (books/LN, free GraphQL) — optional third book provider. | 057 | Optional | M |
| MISSION-065 | Bangumi adapter (CN ACGN incl. LN/WN, open API, ~1 rps) — optional. | 053 | Optional | M |

### M8 · Import / Export (MISSION-066 … 072)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-066 | Import pipeline core: parser→validator→normalizer→dedup→preview→txn→report. | 026,019 | Core | L |
| MISSION-067 | JSON import (app format) + CSV import (mapping UI). | 066 | Core | M |
| MISSION-068 | Bulk-import preview UI (per-item outcome) + confirm/cancel. | 066,040 | Core | M |
| MISSION-069 | Background TaskManager (progress, cancel, typed results) wired to import. | 066 | Core | M |
| MISSION-070 | Export JSON / CSV / Markdown (streaming, dialog pick). | 019 | Core | M |
| MISSION-071 | Provider imports: AniList user export, MAL (Jikan), **Goodreads CSV**, **StoryGraph CSV**, Trakt (optional). | 066,054,058 | Optional | M |
| MISSION-072 | Import/export integration tests + fixture data. | 067..070 | Core | M |

> Note: novels/web novels/light novels have no clean open provider (NovelUpdates: no API, ToS
> forbids scraping; AniList indexes LNs only). Use OpenLibrary/Google Books (+ optional Hardcover/
> Bangumi) for metadata; adopt NovelUpdates' genre/tag taxonomy as conventions. Book imports come
> from Goodreads/StoryGraph CSV (user-owned data). `API_PROVIDERS.md` §12–15.

### M9 · Reviews & Collections (MISSION-073 … 078)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-073 | Review/notes UI (rating, review, short review, spoiler, personal tags) + commands. | 017,042 | Core | M |
| MISSION-074 | Favorites flag in grid/list + filter. | 039 | Core | S |
| MISSION-075 | Collections CRUD + drag/drop membership + bulk add. | 017,040 | Core | M |
| MISSION-076 | Smart collections: save filter as collection; query builder (basic). | 039,075 | Important | M |
| MISSION-077 | Bulk operations on filtered selection (status/tag/list/delete) with summary. | 045 | Important | M |
| MISSION-078 | **Mood / pace / content-warning badges** on detail page (StoryGraph); content warnings as acknowledged metadata. | 073,042 | Optional | M |

### M10 · Stats & Calendar (MISSION-079 … 082)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-079 | StatsService endpoints + Stats page (cards + small charts, tabular numbers). | 027,019 | Important | M |
| MISSION-080 | Calendar: local air/release dates + activity; month grid + list. | 051,039 | Optional | M |
| MISSION-081 | Year-in-review style recap. | 079 | Optional | M |
| MISSION-082 | **Reading recap stats**: pages/chapters per month, mood/pace/format trends (StoryGraph) — local-data-only. | 079 | Optional | M |

### M11 · Backup & Recovery (MISSION-083 … 088)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-083 | BackupService: `VACUUM INTO` snapshot + assets + meta → `.mylore`; validate. | 011 | Core | M |
| MISSION-084 | Restore: quarantine current, swap, verify, rollback-safe. | 083 | Core | M |
| MISSION-085 | Automatic backup schedule + rotation (N + monthly) in preferences. | 083 | Core | M |
| MISSION-086 | Pre-migration auto-backup hook. | 083,012 | Core | S |
| MISSION-087 | Backups UI (list, restore, validate, delete) + recovery from corrupt DB (prompt restore). | 083,084 | Core | M |
| MISSION-088 | Merge UI with conflict preview + restore-from-trash for merges. | 028 | Important | L |

### M12 · UX Polish (MISSION-089 … 094)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-089 | Complete shortcut map + command palette (add, mark complete, status, navigate, settings). | 036 | Important | M |
| MISSION-090 | States audit: empty/loading/skeleton/error/retry on every data surface. | 040..050 | Core | M |
| MISSION-091 | RTL pass: mirror nav, flip icons, mixed-direction titles, test AR+EN. | 033 | Core | M |
| MISSION-092 | Accessibility pass (WCAG AA): focus, labels, dialogs, contrast, reduced-motion. | 037 | Important | M |
| MISSION-093 | Performance pass: virtual lists, debounced search, image cache, startup timing. | 021,040 | Important | M |
| MISSION-094 | Micro-interactions & density tiers (comfortable/compact). | 031 | Optional | M |

### M13 · Testing & Release (MISSION-095 … 099)

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-095 | Integration tests: DB, providers (recorded fixtures), import/export, backup/restore. | all | Core | L |
| MISSION-096 | E2E (Playwright + tauri-driver): add media, search, track progress, import, backup, restore. | 095 | Core | L |
| MISSION-097 | Provider mock harness + fixtures committed (offline CI). | 053..057 | Core | M |
| MISSION-098 | Release pipeline: build installers (Win/mac/Linux), signing (Win/mac), versioning, changelog. | 007 | Core | L |
| MISSION-099 | Alpha → Beta → Stable gates + `MILESTONE-REPORT.md` per milestone. | all | Core | M |

### FX · Future Scope (MISSION-100+)

Post-Stable, behind designed seams. Nothing here blocks M1–M13 (ADR-013 scope discipline).

| Mission | Description | Deps | Pri | Cplx |
|---------|-------------|------|-----|------|
| MISSION-100 | Cloud sync: aggregate-level last-write-wins + conflict resolution (`updatedAt` already designed). | 099 | Optional | L |
| MISSION-101 | Trakt import + scrobble integration. | 099 | Optional | M |
| MISSION-102 | SIMKL import source. | 099 | Optional | M |
| MISSION-103 | Plugins: provider-adapter plugin seam (first plugin surface). | 099 | Optional | L |
| MISSION-104 | AI features (optional, local, disable-able): auto-tag suggestions, summaries. | 099 | Optional | L |
| MISSION-105 | Mobile companion (read-only or lightweight tracking). | 099 | Optional | L |
| MISSION-106 | WN/LN chapter-release notifications + release-feed calendar (NovelUpdates-style Normal mode). | 099 | Optional | M |
| MISSION-107 | Buddy reads with progress-gated spoiler protection (StoryGraph-style; multi-user). | 099 | Optional | L |
| MISSION-108 | New content types: games, podcasts, music (data-only additions: contentType + progress template). | 099 | Optional | M |
| MISSION-109 | NoviList import source (WN/LN tracker with API docs; young, watch). | 099 | Optional | M |
| MISSION-110 | ISBNDB paid enrichment fallback (free tier: 100 req/mo). | 099 | Optional | S |
| MISSION-111 | SQLCipher encrypted database (opt-in). | 099 | Optional | L |
| MISSION-112 | Advanced visualizations/chart library for stats. | 099 | Optional | M |

---

## 4. Execution workflow (per mission)

1. **Pick up** a mission in the current milestone (only READY/BACKLOG missions; never skip deps).
2. Create its checklist in the epic folder (from the template in `DEVELOPMENT_PLAN.md §5`).
3. **Implement → test → review → fix → update docs → set status** (spec §90, §91).
4. Run quality gates for the milestone (TypeScript PASS · Rust build PASS · Lint PASS · Tests
   PASS · Migrations PASS · no console/TS/lint errors · no broken deps).
5. Mark mission DONE only when its acceptance criteria pass; update this roadmap's status.

## 5. Definition of Done & Quality Gates

Implementation ✓ · Type safety ✓ · Tests ✓ · Error handling ✓ · UI validation ✓ · Docs updated ✓ ·
A11y where needed ✓ · Perf where needed ✓ · No console/TS/lint errors ✓ · No broken deps ✓.

Release gates (M13): TS PASS · Rust build PASS · Lint PASS · Tests PASS · Migrations PASS ·
Import/Export PASS · Backup/Restore PASS · Critical UX flows PASS · Security review PASS.

## 6. MVP scope (what ships as "Stable 1.0")

M1–M6 + M7 core providers (AniList, TMDB, MangaDex, OpenLibrary) + M8 (JSON/CSV import-export,
Goodreads/StoryGraph CSV) + M9 basics + M11 backups. Concretely: **Media CRUD · Library ·
Search(local) · Tracking (incl. novel chapter UX) · Provider import/search · Reviews · Tags ·
Collections · Backup/Restore** — fully offline, en/ar RTL.

## 7. Doc roles

| Doc | Role |
|-----|------|
| **ROADMAP.md** (this) | Master: milestones + complete mission list + status. |
| `DEVELOPMENT_PLAN.md` | Reference: per-task detail (files, tests, AC) + traceability. |
| `PROJECT_REQUIREMENTS.md` | What we build (requirements, priorities). |
| `PHASE0_REPORT.md` | Executive summary of Phase 0. |
| `RESEARCH.md` · `API_PROVIDERS.md` · `UX_RESEARCH.md` | Why we build it that way. |
| `ARCHITECTURE.md` · `DOMAIN_MODEL.md` · `DATABASE.md` · `DECISIONS.md` · `DESIGN_SYSTEM.md` | How. |
