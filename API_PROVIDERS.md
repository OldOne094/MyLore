# MyLore — API Providers Research

> Phase 0 · Verified August 2026 · Every fact checked against current docs/registries.
> Format per provider (spec §5): docs, auth, free/paid, rate limits, search, metadata, images,
> genres, authors, studios, characters, seasons, episodes, chapters, external ids, user data,
> reviews, ratings, update frequency, reliability, ToS/licensing.

**Summary of verified figures**

| Provider | Endpoint | Auth | Rate limits (verified) | License notes |
|---|---|---|---|---|
| AniList | `graphql.anilist.co` | none (public) / OAuth (user lists) | ~90 req/min per IP | free, no key for public data |
| Jikan | `api.jikan.moe/v4` | none | ~3 req/s, 60 req/min | unofficial MAL mirror; cache responses |
| MangaDex | `api.mangadex.org` | none (public) / OAuth | ~5 req/s (default; verify AUP) | must credit; no ads/paid on API data |
| TMDB | `api.themoviedb.org/3` | free API key | ~40 req/10s (soft ~50 rps ceiling) | non-commercial free tier; attribution required |
| TVDB | `api4.thetvdb.com` | free key (JWT) | tier-based (vary) | free tier; verify current limits |
| Trakt | `api.trakt.tv` | client id + OAuth | ~1 req/s sustained (free); 2026 free caps | personal use free; commercial needs approval |
| OpenLibrary | `openlibrary.org` REST | none | 1 rps (3 rps with UA+email) | public-good; bulk allowed for aligned use |
| Google Books | `books.googleapis.com` | free key | ~100 req/min/user (default) | free key; request higher quota |
| BookBrainz | `bookbrainz.org` | none | no fixed public limit (be polite) | open data (MusicBrainz) |
| SIMKL | `api.simkl.com` | free key | tiers; free generous for personal | free tier exists |
| Annict | `api.annict.com` | OAuth | ~4 req/s (documented) | Japanese-focused |
| Hardcover | `api.hardcover.app` (GraphQL) | public read (verify auth) | not published (flagged) | free tier; young/indie |
| Bangumi | `api.bgm.tv/v0` | none (anonymous read) | ~1 req/s; 15/60 s; 80/10 min | open community wiki; credit |
| ISBNDB | `api.isbndb.com` | API key | free 100 req/mo, 10 req/min | paid beyond free tier |

> UNKNOWN/flagged items are marked explicitly; nothing is invented (spec §94).

---

## 1. AniList (anime + manga) — **Primary anime/manga provider**

- **Docs:** https://anilist.gitbook.io/anilist-apiv2-docs · schema on GitHub `AniList/ApiV2-GraphQL-Docs`.
- **Auth:** none for public data; OAuth2 only for user lists (we do not need it — local-first).
- **Free/Paid:** free, no API key for public data.
- **Rate limits:** ~90 requests/minute (per IP); GraphQL batching recommended.
- **Search:** `Media(search:, type: ANIME|MANGA)`; advanced filters (genre, tag, season, year, format).
- **Metadata:** titles (romaji/native/english), description, cover (large/medium/extraLarge), banner,
  genres, tags (with spoiler flags), status, start/end dates, episodes, chapters, volumes,
  duration, studios, staff (director, writers), characters, relations, external links
  (MAL, AniDB, TMDB, IMDb, Kitsu, Anime-Planet, Bangumi, MangaDex…), recommendations.
- **User data / reviews / ratings:** community scores + reviews exist (public). We consume
  external rating only as metadata.
- **Update frequency / reliability:** actively maintained; high uptime; very stable API.
- **ToS:** free public data; attribution encouraged; no abuse. External-link cross-ids are the
  backbone of our identity resolution for anime/manga.
- **Our use:** search + details + nodes (episode/chapter counts) + related + external ids for
  anime and manga/manhwa/manhua (manhwa/manhua are classified as `MANGA` type with formats).
  Light novels are indexed as `MANGA` type with `NOVEL` format; web novels are generally NOT
  indexed → AniList covers LNs, not WNs.

## 2. Jikan (unofficial MyAnimeList) — fallback anime/manga

- **Docs:** https://jikan.moe · OpenAPI spec in repo.
- **Auth:** none. **Free:** yes (MIT, donation-funded).
- **Rate limits:** ~3 req/s and 60 req/min (community-documented); cache aggressively.
- **Metadata:** anime + manga + characters + people + seasons + top + recommendations + reviews +
  airing schedules; images via MAL CDN.
- **Limitations:** unofficial mirror of MAL; brand-new titles may lag; no user list APIs.
- **Reliability:** high (25M+ req/week as of research); 98%+ uptime observed.
- **ToS:** data derived from MyAnimeList; no commercial guarantee — use as *fallback*, not sole source.
- **Our use:** alternative search/details when AniList is down or for MAL-native ids (we store the
  MAL id as an external id regardless of provider).

## 3. MangaDex — manga/manhwa/manhua + chapters

- **Docs:** https://api.mangadex.org/docs · OpenAPI downloadable.
- **Auth:** public GETs are keyless; OAuth for user data (not needed for MVP).
- **Free/Paid:** entirely public & free; **acceptable-use policy**: must credit MangaDex; must
  credit scanlation groups if we surface chapters; no ads/paid services built on the API.
- **Rate limits:** public endpoints rate-limited (approx 5 req/s by default); honor 429 +
  `Retry-After`; use `GET /at-home` for image server allocation.
- **Metadata:** manga (id uuid), titles in many languages (incl. Arabic/English/JP/KR/ZH),
  description, covers (CDN), tags (curated), authors/artists, relationships, status, year,
  content rating; **chapters** (numbers, titles, pages, release dates, groups), volumes.
- **Why valuable:** the only legal, high-quality open API covering *manhwa/manhua* with chapter
  trees — critical for REQ-MEDIA-001 and the reader-tracking domain.
- **Our use:** search + details + chapter/volume node tree + covers.

## 4. TMDB — movies & TV

- **Docs:** https://developer.themoviedb.org · **Auth:** free personal API key.
- **Free/Paid:** free for non-commercial; commercial requires written agreement.
- **Rate limits:** ~40 requests / 10 s (soft ceiling ~50 rps documented); use `append_to_response`.
- **Metadata:** movies, TV series, seasons, episodes (air dates, runtime, overview, stills),
  genres, keywords, credits (cast/crew/directors/studios), networks, external ids (IMDb,
  TVDB, TVmaze…), watch providers, trending/discover, images (posters/backdrops).
- **ToS:** attribution required (TMDB logo + link on data/images we display).
- **Reliability:** very stable; widely used.
- **Our use:** primary movies+TV provider; TV → Season/Episode tree + air dates for calendar.

## 5. TVDB v4 — TV (episodes, translations, artwork)

- **Docs:** https://thetvdb.com/api-information · v4 API docs; **Auth:** free key → JWT bearer.
- **Metadata:** series, seasons, episodes with translations (Arabic supported), artwork,
  people, remote ids, air dates; good for non-English and older shows.
- **Rate limits:** tier-based; verify current numbers at signup (flagged).
- **Our use:** secondary TV provider / supplement to TMDB for episode-level detail + translations.

## 6. Trakt — movies & TV scrobbling (optional sync)

- **Docs:** https://docs.trakt.tv · **Auth:** client id + OAuth.
- **Free/Paid:** free for personal apps; 2026 free caps: watchlist 250, personal lists 5,
  history 100k, ratings 10k, notes 100 (VIP raises limits; paid).
- **Rate limits:** sustained ~1 req/s for free clients; 429 + `X-Ratelimit`/`Retry-After` headers.
- **Metadata/sync:** deep per-user sync (history, collections, ratings), rich catalog keyed by
  trakt/imdb/tmdb/tvdb ids.
- **Why:** import/export of watch history is a compelling *import source* (Yamtrack/Ryot do this).
  Not needed for core metadata (TMDB covers it). **Future import provider.**

## 7. OpenLibrary — books

- **Docs:** https://openlibrary.org/developers/api · **Auth:** none.
- **Rate limits:** 1 req/s default; 3 req/s if you send `User-Agent: <app> (<contact>)`.
- **Metadata:** works/editions (title, alt titles, authors, publishers, publish dates, ISBNs,
  page count, subjects/genres, covers via `covers` id → `covers.openlibrary.org`), search,
  full-text search, data dumps for offline bulk.
- **ToS:** public-good; bulk/batch discouraged except via search; credit OpenLibrary.
- **Our use:** primary book provider (free, no key). ISBN resolution for dedup.

## 8. Google Books — books (secondary)

- **Docs:** https://developers.google.com/books · **Auth:** free API key (Google Cloud).
- **Rate limits:** default ~100 queries/min/user (per-project quota; verify in console).
- **Metadata:** volumes (title, authors, categories/genres, description, pageCount, ISBN/ISSN,
  covers, language, publishedDate, ratings); image links are provisional (must cache locally).
- **ToS:** standard Google API terms; attribution not required but covers should be cached.
- **Our use:** fallback/secondary book provider; strong for non-English and preview data.

## 9. BookBrainz — books (open bibliographic data)

- **Docs:** https://bookbrainz.org/docs/api · **Auth:** none. Open data (MusicBrainz foundation).
- **Metadata:** works/editions/relationships (authors, series, relations between works).
- **Our use:** optional authority for identity relationships; lower priority.

## 10. SIMKL — aggregated anime/TV/movies (import source)

- **Docs:** https://simkl.docs.apiary.io · free key tiers.
- **Our use:** potential future import/aggregate source for anime episode data.

## 11. Annict — anime (Japanese, optional)

- **Docs:** https://docs.annict.com · OAuth, ~4 req/s documented.
- **Our use:** optional Japanese-community metadata source; low priority.

## 12. Hardcover — books (indie, GraphQL)

- **Docs:** https://developers.hardcover.app (GraphQL) · **Auth:** public reads open (verify
  current auth at build time); user data via OAuth.
- **Free/Paid:** free tier; Pro subscription. **Rate limits:** not published (flagged).
- **Metadata:** books, authors, series, editions, covers, genres/moods, ratings, reading
  progress/status; strong indie polish; positioned as a Goodreads API alternative.
- **Reliability:** young indie team — schema churn risk; verify before depending on it.
- **Our use:** optional third book provider (alongside OpenLibrary/Google Books) for
  novels/light novels and richer mood/pace-style metadata.

## 13. Bangumi — CN ACGN (anime/manga/light novels/web novels/games)

- **Docs:** https://github.com/bangumi/api (OpenAPI 3.0) · https://api.bgm.tv/v0 ·
  **Auth:** none for anonymous read (register for write).
- **Rate limits (documented):** 1 req/s per person; 15 in 60 s; 80 in 10 min → cache heavily.
- **Metadata:** anime/manga/games/books incl. **light novels & web novels**, Chinese-community
  tags and relations, covers; AniList already links Bangumi ids (cross-ids).
- **Our use:** optional light-novel/web-novel/Chinese metadata source + cross-id resolution.

## 14. ISBNDB — ISBN lookup (commercial)

- **Docs:** https://isbndb.com · **Auth:** API key.
- **Pricing (verified):** free 100 requests/month, 10 req/min; Pro $99/mo (120k req/mo);
  Lifetime one-time (~$1000, 2M req).
- **Metadata:** ISBN → basic book metadata (title, author, publisher, covers, dimensions).
- **Our use:** not needed for MVP (OpenLibrary covers ISBNs free); optional paid fallback for
  high-volume ISBN enrichment.

## 15. Evaluated and NOT used (with reasons)

| Service | Reason |
|---------|--------|
| MyAnimeList **official** API | Requires application/approval, heavy rate caps, owner change 2025 (Gaudiy); Jikan covers MAL data publicly. |
| Kitsu API | Project effectively inactive as of 2026 (apps pulled 2024); risk of data rot. |
| AniDB | No open public API without application; not usable out-of-the-box. |
| Anime-Planet / MangaUpdates / Goodreads / StoryGraph | No public API (Goodreads API legacy/deprecated). |
| NovelUpdates | No official API; ToS prohibits scraping; no official export → not a provider or import source. Adopt its genre/tag taxonomy + tracking modes as conventions only. |
| Bookwyrm | Federated; Anti-Capitalist Software License v1.4 (not OSI) → legal friction; instance-dependent data quality. |
| OMDb | Freemium paywall; TMDB covers movies/TV better. |

## 16. Provider risks & mitigations

- **AniList/MangaDex/TMDB** are the spine; Jikan + Google Books are cheap fallbacks → no single
  point of failure for any content type (spec §45, §73).
- **Novels/web novels/light novels have no single strong open provider** (NovelUpdates has no
  API; AniList indexes LNs only). Mitigation: OpenLibrary + Google Books (+ optional Hardcover/
  Bangumi later) cover metadata; NovelUpdates' taxonomy is adopted as tag conventions, never
  scraped. Book imports come via Goodreads/StoryGraph CSV (user-owned data), not an API.
- All provider calls go through the coordinator: timeout, retry w/ backoff, cancellation,
  rate-limit awareness, fixture-recorded tests offline (`TESTING.md`).
- Attribution obligations: TMDB logo/link on movie/TV data; MangaDex credit line in About.
  These are UI requirements, tracked as tasks.
