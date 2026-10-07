# MyLore — Competitor & Product Research

> Phase 0 · Research Report · August 2026
> Input to: `UX_RESEARCH.md`, `ARCHITECTURE.md`, `DECISIONS.md`

---

## 1. Method

Studied cloud trackers (web/mobile), self-hosted trackers, media-server integrations, and
reading platforms. Focus: UX patterns, navigation, data model, import/export, offline behavior,
API support. We adopt **ideas, never assets or branding** (spec §97).

## 2. Cloud / web trackers

### MyAnimeList (MAL)
- Type: anime + manga tracking community · Web/mobile · Free (ads), proprietary.
- Strengths: largest catalog & community; manga parity; seasonal charts; recommendations;
  review culture.
- Weaknesses: dated UI; slow heavy pages; list ops require many clicks; no offline mode;
  official API gated; acquired by Gaudiy (Web3/AI) 2025 → platform-risk signal.
- UX patterns: status tabs (Watching/Completed/…) as primary list nav; per-item
  score/progress/episodes; seasonal page.
- Data model: Anime/Manga/Character/Person/Studio; user list = status + score + episode/chapter.
- Import/export: CSV/XML exports exist; API read via Jikan (unofficial).
- **Ideas to borrow:** status-tab navigation; seasonal charts; per-item quick progress.

### AniList
- Type: anime + manga tracker · Web · Free.
- Strengths: modern UI; excellent public GraphQL API (no key); customizable list layouts
  (grid/list); strong data relations; good community.
- Weaknesses: web-only ergonomics; no desktop app; some list interactions still multi-step.
- UX patterns: filter bar (format/status/score/year/genre), card grid with progress overlay,
  detail page with big cover + organized meta sections.
- **Ideas to borrow:** rich filter bar; card-with-progress-overlay; clearly-sectioned detail page;
  API-first data quality.

### Anime-Planet
- Type: discovery-first anime tracker · Web · Free.
- Strengths: deep tag system (mood/setting/themes), recommendation threads, seasonal charts.
- Weaknesses: dated UI, thin tracking tools, no API.
- **Ideas:** domain tags beyond genre (mood/theme) → our personal/domain tag model.

### Kitsu
- Type: anime + manga tracker · Web/mobile.
- **Status 2026: effectively inactive** (apps pulled 2024, dev stalled). Decision: exclude as a
  provider; still a data point for why APIs without maintenance rot.

### Trakt
- Type: movies/TV scrobbling + social · Web/mobile/integrations · Freemium (VIP paid).
- Strengths: automatic scrobbling from media servers; deep history; calendar ("upcoming
  episodes"); stats; rich cross-ids.
- Weaknesses: 2026 free limits (250 watchlist, 5 lists, 100k history); social-first direction.
- **Ideas:** automatic sync from media servers (future); calendar of upcoming releases;
  activity/history timeline.

### SIMKL
- Type: TV/movies/anime tracker · Web/mobile · Freemium.
- Strengths: multi-source; anime+TV in one; watchlists; notifications.
- **Ideas:** combined TV+anime list model; notification stream (future, optional).

### Letterboxd / Serializd
- Movies (Letterboxd), TV (Serializd). Strengths: journal-style logs (date watched), yearly
  recaps, taste stats. Weaknesses: per-domain silos.
- **Ideas:** journal/watched-date entries → our `activity` log + calendar; year-in-review style
  stats (optional).

## 2b. Books, novels, web novels & light novels

### Goodreads (books)
- Type: book tracking + social + catalog · Web/mobile · Free (ads), proprietary (Amazon).
- **Status 2026: official API effectively dead** — legacy/deprecated, no new keys for third-party
  apps. The usable data path is the user **CSV export** (title, author, ISBN, shelves,
  exclusive shelves, date added/read, rating, review text) → our import (REQ-IMPORT).
- Strengths: largest book community; **shelves** as flexible lists; "currently reading" status;
  yearly reading challenge.
- Weaknesses: dated UI, no offline, ads, walled catalog.
- **Ideas to borrow:** shelves as first-class lists (= our collections); currently-reading =
  in_progress status tab; CSV as the import bridge.

### StoryGraph (books)
- Type: book tracking + discovery · Web/mobile · Free/premium · ~5M signups (2026).
- **No public API** — data only via scraping (ToS-hostile) or user CSV export (import path).
- Strengths: best-in-class book-tracker UX: **mood tags, pacing, content warnings, DNF
  (did-not-finish) with % progress**, granular ratings, rich reading stats (mood/pace/format
  trends, monthly), one-click Goodreads CSV import, buddy reads with **progress-gated spoiler
  protection**, journal-style "currently reading".
- Weaknesses: no API, no offline, social features require an account.
- **Ideas to borrow:** mood/pace/content-warning metadata; DNF-with-progress; reading stats;
  buddy-read spoiler gating (future multi-user); story-graph genre model.

### NovelUpdates (web novels & light novels)
- Type: the definitive directory + reading-list tracker for EN-translated web novels (CN/KR/JP:
  wuxia, xianxia, cultivation, isekai, otome…) · Web · Free, ad-heavy.
- **No official public API**; ToS prohibits automated scraping and third-party scraping APIs are
  fragile. The LNReader plugin (`github.com/lnreader/lnreader-plugins`), maintained by an NU
  moderator, publishes authoritative HTML-scrape selectors for search/details/chapter-tree — we
  follow those at a modest rate as a metadata provider (MISSION-065), not for reading content (NU
  hosts none).
- Strengths: unrivaled WN/LN catalog + taxonomy (**genres** incl. wuxia/xianxia/cultivation,
  **tags** incl. smut/shoujo-ai/yuri, **translation status**, **chapter release feed** with
  dates), reading list with **Normal mode** (auto-track chapters via release feed) vs
  **Manual mode** (user sets current chapter), custom lists, private tags/notes, ratings,
  reviews. ~55M visits/mo.
- Weaknesses: dated UI, ads, no API, no offline, no official export.
- **Ideas to borrow:** Normal vs Manual tracking modes → our auto vs manual node progress;
  chapter release feed → calendar/notifications (future); WN/LN genre/tag taxonomy; per-chapter
  read-status rows in a chapter list.
- Alternative: **NoviList** (new WN/LN tracker, NovelUpdates import, public API docs) — too young
  to depend on; watch as a future provider/import source.

### Hardcover (books, indie)
- Type: modern book tracker + database · Web/mobile · Free + Pro.
- **Public GraphQL API (free)** — marketed as "the Goodreads API alternative"; open model, no key
  for public reads (verify current auth at build time). Young indie team → serialization churn risk.
- **Ideas:** candidate book metadata provider alongside OpenLibrary/Google Books; low-cost to try.

### Bookwyrm (books, federated)
- Type: open-source, ActivityPub-federated book tracker; collaborative book database across
  instances. Django+Postgres+Redis; **Anti-Capitalist Software License v1.4** (not OSI) — code
  reuse needs legal review. Instance data is federated; usable as a metadata source in principle,
  but reliability varies by instance.
- **Ideas:** collaborative catalog concept — we stay local-first instead.

### Bangumi (CN ACGN — anime/manga/light novels/web novels/games)
- Type: community wiki + tracker (bgm.tv) · Web · Free, no ads.
- **Open public API** (`api.bgm.tv/v0`, OpenAPI 3.0 spec, anonymous read OK, ~1 req/s).
  Covers anime/manga/games/books incl. **light novels & web novels**; strong Chinese-community
  data, tags, relations (AniList already links Bangumi ids).
- **Ideas:** optional light-novel/web-novel/CN metadata source + cross-ids; cache heavily (~1 rps).

## 3. Self-hosted trackers (closest architectural relatives)

### Ryot (ignisda) — Rust + TS, PostgreSQL
- One self-hosted app for movies/TV/anime/manga/books/games/fitness; imports from
  Goodreads/Trakt/MAL/Audiobookshelf + scrobble from Jellyfin/Plex/Kodi/Emby; metadata from
  TMDB/OpenLibrary/IGDB/Audible; GraphQL API; collections; reviews; rich stats.
- Weaknesses: server+Postgres (not offline, not single-binary desktop); fitness scope creep.
- **Ideas:** importer architecture (Goodreads/Trakt/MAL as import *sources*); metadata provider
  wiring; unified collection/review model; strong stats.

### Yamtrack — Django, SQLite or Postgres
- Movies/TV/anime/manga/games/books/comics/board games; per-season episode tracking; custom
  media entries; personal lists; calendar w/ iCal subscription; Apprise notifications; imports
  from Trakt/Simkl/MAL/AniList/Kitsu; CSV export/import; Jellyfin/Plex/Emby integration;
  multi-user + OIDC.
- **Ideas:** custom entries for niche media (our manual add + later enrichment); per-season
  tracking UX; calendar + iCal export (optional); CSV round-trip import/export.

### Watcharr / Scrob
- Watchlist-style, clean UIs; Scrob = Letterboxd+Trakt for Jellyfin/Plex/Emby.
- **Ideas:** clean minimal UI direction; activity/history wall.

### MediaTracker / Tome / Calibre-ecosystem (Kavita, Komga)
- MediaTracker: old self-hosted movie/TV/books tracker. Tome: Calibre-backed book tracker.
  Kavita/Komga/Calibre: *reading servers* (store actual files + progress) — adjacent domain,
  not trackers.
- **Ideas (from readers):** per-book page progress + "continue reading" resume UX; library
  reading position persistence.

### Mihon / Tachiyomi
- Android manga *reader* with tracker sync (AniList/MAL/MangaDex/…). Deep progress UX.
- **Ideas:** one-tap "next chapter" and auto-mark-read flows; tracker multi-sync as an import path.

## 4. Cross-cutting UX findings (→ `UX_RESEARCH.md`)

1. **Status as primary filter/tab** — universal across MAL/AniList/Trakt/Yamtrack.
2. **Progress capture must be ≤1–2 actions**; grid cards with progress overlays win.
3. **Detail pages** = hero cover + meta blocks + user panel; personal vs metadata visually separate.
4. **Filters are better than tabs** for power users; sort + group in library grids.
5. **Import stories matter** (MAL/Goodreads/Trakt) — biggest onboarding lever.
6. **Offline behavior:** none of the web trackers work offline; Ryot/Yamtrack need a server.
   This is our **differentiator**: single-binary, fully local, private.
7. **Stats & calendar** appear in all mature trackers; cheap to do correctly with an activity log.
8. **Multi-user/social/features creep** is the #1 killer of tracker scope → our MVP stays personal.
9. **Book & web-novel trackers confirm the same laws** (Goodreads/StoryGraph/NovelUpdates):
   lists, one-tap progress, per-chapter read trees, stats. StoryGraph adds mood/pace/content
   warnings; NovelUpdates adds Normal-vs-Manual chapter modes and a release feed → calendar.
10. **The import story extends to books:** Goodreads and StoryGraph both export CSV (title, ISBN,
    rating, shelves, dates, review) → one shared book-CSV import path is a major onboarding lever.
11. **No open provider serves web novels/light novels** (NovelUpdates has no API; AniList indexes
    light novels only). Strategy: a NovelUpdates HTML-scrape adapter (selectors from the LNReader
    plugin, an NU-moderator project; MISSION-065) for WN/LN search/details/chapters, plus books
    providers (OpenLibrary/Google Books) + AniList-LN fill metadata; NovelUpdates taxonomy adopted
    as tag conventions.

## 5. Positioning statement

MyLore = the **local-first, offline, private** alternative: single desktop app, own database,
no account, no ads, metadata from multiple open APIs, import/export from the major trackers —
with AniList/MangaDex/TMDB/OpenLibrary-grade data quality (research §3) instead of a walled silo.

## 6. Sources

- Ryot: github.com/IgnisDa/ryot, ryot.io, OpenAltFinder record (v10.4.0, active).
- Yamtrack: github.com/FuzzyGrim/Yamtrack, docs.
- Trakt 2026 limits: forums.trakt.tv thread (Feb 2026) + docs.trakt.tv rate-limiting.
- 2026 anime tracker comparisons (Achriom reviews, blog.oriz.in) for MAL/AniList/Kitsu/Anime-Planet status.
- MAL ownership change (Gaudiy, May 2025) reported in 2026 comparisons.
- Goodreads API status: goodreads.com/api (legacy/deprecated), 2026 community reports; CSV export format.
- StoryGraph: thestorygraph.com (features, ~5M signups 2026); no public API.
- NovelUpdates: novelupdates.com (reading-list modes, genres/tags, ~55M visits/mo); no API — HTML-scrape selectors adopted from the LNReader plugin (github.com/lnreader/lnreader-plugins, maintained by an NU moderator).
- Hardcover: hardcover.app GraphQL API docs ("Goodreads alternative").
- Bookwyrm: bookwyrm.social (Anti-Capitalist Software License v1.4).
- Bangumi: github.com/bangumi/api (OpenAPI 3.0 spec, api.bgm.tv, ~1 req/s rate limits).
- ISBNDB: isbndb.com/pricing (free 100 req/mo, 10 req/min; Pro $99/mo).
- NoviList: novilist.com + github.com/novilist/api-docs (young WN/LN tracker with API docs).
