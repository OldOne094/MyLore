# MyLore — Domain Model

> Phase 0 · Domain Model Proposal · August 2026
> Companion docs: `ARCHITECTURE.md`, `DATABASE.md`, `API_PROVIDERS.md`

---

## 1. Design Decision: Composition over Inheritance

We compared **inheritance hierarchies** (`Book : Media`, `Anime : Media`), **tagged unions**
(`type Media = Book | Anime | …`), and **composition / single entity + discriminators**.

**Decision: single `Media` entity + `contentType` discriminator + capability flags.**
Hierarchical variation is handled by a **generic content-node tree** (see §4), not by subclassing.

Rationale:
- SQLite has no inheritance; object-oriented mapping would leak into persistence.
- A tagged union in TypeScript is fine at the API boundary, but every media type shares ~90% of
  fields; a discriminated union adds churn without domain value.
- Progress differs *per unit kind* (pages / chapters / episodes), not per media type; the unit is
  the discriminated value, not the media.
- New media types (podcast, game, music) become new `contentType` values + a unit kind + a
  "progress template" — no schema/class redesign. This directly satisfies the extensibility
  requirement (P4, REQ-MEDIA-001).

The `Media` entity owns **metadata**. User-owned data lives on separate aggregates
(`TrackingState`, `Review`, `Tag`, `Collection`) so metadata refresh can never clobber it (P3).

---

## 2. Core Entities

### 2.1 Media (metadata)

```
Media
 ├─ id: MediaId (UUID, internal)
 ├─ contentType: 'book'|'novel'|'web_novel'|'manga'|'manhwa'|'manhua'|'anime'|'tv'|'movie'|'other'
 ├─ format: optional refinement (e.g. light_novel, webtoon, ova, special, manhwa_colored)
 ├─ title: { main, original, alt[] }   (per-language display resolution)
 ├─ description (synopsis)
 ├─ status: 'announced'|'ongoing'|'completed'|'hiatus'|'cancelled'|'unknown'   (publication/airing status)
 ├─ dates: { startDate, endDate, releaseYear }
 ├─ language / country / contentRating
 ├─ runtime: { pages, durationMinutes, episodeCountEstimate, chapterCountEstimate }  (optional aggregates)
 ├─ cover / banner: asset refs (§ 2.8)
 ├─ people: authors[], artists[], directors[], studios[], publishers[], networks[]   (roles)
 ├─ genres[], tags[] (community/domain tags)
 ├─ externalIds: Map<Provider, { id, url }>
 ├─ relationships[]: related media (sequel/prequel/adaptation/same universe) + strength
 └─ provenance: provider + last metadata refresh
```

Guiding rule (spec §10): *do not add fields without real use*. Each field above maps to at least
one provider field and one UI surface.

Note on novel coverage (from research): light novels are `novel` contentType with
`format=light_novel`; translated web novels are `web_novel`. Genre/taxonomy tags for these
domains (wuxia, xianxia, cultivation, isekai, slow-burn, smut) and content-warning-style tags are
`tags`/`genres`, **never** content types — matching NovelUpdates/StoryGraph conventions. Chapter
release dates on `ContentNode.releaseDate` feed the CalendarService for ongoing WN/LN; a
per-media "auto-track via release feed" flag on Tracking mirrors NovelUpdates' Normal vs Manual
mode.

### 2.2 Content Node (generic hierarchy)

A single node type models every hierarchy and also flat media:

```
ContentNode
 ├─ id (UUID)
 ├─ mediaId
 ├─ parentId?            → tree (Series → Season → Episode; Manga → Volume → Chapter)
 ├─ kind: 'season'|'episode'|'volume'|'chapter'|'page_range'|'track'|'issue'|'node'
 ├─ position: int        → ordering within parent (chapter 12, ep 3, vol 2)
 ├─ title?, number?, releaseDate?, duration?, pageCount?, synopsis?
 ├─ externalId? (provider node id)
 └─ isSpecial? (special/episode 0, omake, extra)
```

Why a tree instead of four tables: seasons/episodes for TV, volumes/chapters for manga/novels,
and future types (podcast episodes, game levels) all reduce to *kind + parent + position*.
Progress is therefore **uniform**: one table links a user to a node. This avoids the
"per-media-type tracking hacks" the spec forbids (§11).

### 2.3 Tracking (user state per media)

```
Tracking
 ├─ mediaId (unique per media in MVP; per-user later)
 ├─ coreStatus: 'planned'|'in_progress'|'completed'|'on_hold'|'dropped'|'re_read'|'re_watch'|'wishlist'
 ├─ customStatusId?        (user-defined status, §2.4)
 ├─ startedAt / finishedAt
 ├─ repeatCount (re-read/re-watch)
 ├─ lastPosition: { nodeId?, unitCount? }   (fast "current chapter/episode")
 ├─ autoTrack?: bool   (auto-mark released nodes read — NovelUpdates "Normal" mode; default false)
 ├─ favorites?: on Review (see §2.6)
 └─ progress is DERIVED by aggregating node states — never stored twice (REQ-TRACK-004)
```

**Node progress** (per episode/chapter/volume):

```
NodeProgress
 ├─ nodeId
 ├─ state: 'unwatched'|'watched'|'read'|'skipped'|'partial'
 ├─ watchedAt / readAt
 ├─ note?
 └─ rating? (per-node, optional)
```

**Progress templates** (per contentType) define which units count and how the aggregate is
computed (pages vs chapters vs episodes). This is data, not code.

### 2.4 Statuses

- `status` table with `isSystem` flag. Core statuses are seeded and semantically meaningful
  (they drive dashboards, stats, auto-transitions). Users may add custom statuses which are
  grouped under a core bucket for behavior.
- Auto-transition rules (e.g. marking all episodes watched → status completed) are explicit and
  reversible, never hidden.

### 2.5 Review & Personal Notes

```
Review
 ├─ mediaId (one per media in MVP)
 ├─ rating: int 1..10 (or null)          → USER rating (distinct from external)
 ├─ review: long text (spoiler-flag-able)
 ├─ shortReview: one-liner
 ├─ notes: free-form personal notes
 ├─ favorite: bool
 ├─ isSpoiler: bool
 ├─ tags: personal tags (free-form, user-owned)
 └─ createdAt / updatedAt
```

External rating/review are **metadata** (stored on `Media.externalRating` + fetched reviews are
transient and clearly labelled); user rating/review are **personal data**. The UI never mixes them
(REQ-REVIEW-002).

### 2.6 Tags & Genres

- `tag` (domain/community tags from providers, e.g. "isekai", "slow-burn"), `genre`
  (broad categories), and **personal tags** (user-created). Three distinct namespaces so provider
  sync and personal taxonomy never collide.
- Tag → media is many-to-many with provenance (source provider) and scope (public/personal).

### 2.7 Collections, Lists, Smart Lists

- `collection` = user-curated list (manual membership, ordering).
- `smart_collection` = persisted filter definition (structured JSON today; query-builder UI later).
  Membership is derived at query time (REQ-COLL-001).
- A media can belong to many collections; favorites is a first-class flag, not a collection.

### 2.8 Assets (images)

```
Asset
 ├─ id (UUID) + kind: 'cover'|'banner'|'avatar'|'node_image'
 ├─ remoteUrl? (original source)
 ├─ localPath? (cached copy under app data image cache)
 ├─ status: 'remote'|'cached'|'failed'|'missing'
 ├─ mimeType, width, height, etag?, lastFetchedAt
 └─ attribution? (provider/credits)
```

Strategy: keep the remote URL, download to a managed local cache, serve from disk; broken links
are tracked and retried lazily (spec §23). Cache has size/expiry/cleanup policy in preferences.

---

## 3. Identity, Deduplication, Merging

### 3.1 Identity Resolution (does this entity already exist?)

Two layers, kept separate (spec §18):

1. **Exact identity:** unique `(provider, externalId)` — strong, automatic.
2. **Fuzzy identity:** normalized-title matching (case-fold, unicode-fold, script-aware
   transliteration table, alternative titles, canonical ISBN/ASIN/IMDb/AniList/AniDB/TMDB ids),
   scored and surfaced for confirmation.

A central `media_identity` table maps a media to every provider id it has ever been seen under.
Import/search consults this before inserting → no duplicates (REQ-MEDIA-005).

### 3.2 Merge

- Merge = combine two identities into one surviving record with an explicit **conflict report**
  (per-field: keep A / keep B / custom) and a **before-image** stored in trash/undo so the merge
  is reversible (REQ-MEDIA-006, REQ-MEDIA-007).
- Node trees, reviews, collections and progress of the absorbed record are re-parented to the
  survivor inside one transaction.

---

## 4. Domain Services (application layer, no UI)

| Service | Responsibility |
|---------|----------------|
| MediaService | CRUD, enrichment orchestration, merge |
| TrackingService | node progress updates, aggregates, status transitions, repeats |
| SearchService | local FTS query building, result ranking, local-vs-external merge |
| IdentityService | exact + fuzzy dedup, title normalization, external-id resolution |
| CollectionService | collections + smart collections |
| ReviewService | user rating/review/notes/tags |
| StatsService | pure functions computing statistics from tracking/media (testable) |
| ImportService / ExportService | pipelines (see `ARCHITECTURE.md`) |
| BackupService | backup/restore/validation/rotation |
| ProviderCoordinator | provider capability routing, rate-limit scheduling, retries (see `ARCHITECTURE.md`) |
| CalendarService | local schedule from release dates + activity |

Domain services are pure logic where possible (no SQL, no I/O); repositories (infrastructure)
implement persistence. Domain logic is shared-testable in Rust and mirrored by thin TS types at
the IPC boundary.

---

## 5. Value Objects

`MediaId`, `ProviderId` (e.g. `anilist`, `tmdb`, `openlibrary`), `ExternalId { provider, value }`,
`Title`, `Genre`, `Tag`, `Rating(1..10)`, `ProgressPosition`, `DateOnly`, `LanguageCode`,
`AssetRef`, `SpoilerFlag`. Values are immutable; entities carry identities.

---

## 6. Domain Invariants

- A media must have at least one title.
- `Tracking.coreStatus` must be one of the seeded core set or mapped from a custom status.
- Node progress `state=completed` implies `watchedAt/readAt` set.
- A node's parent chain must belong to the same `mediaId` (checked on insert).
- External ratings are never written into user review fields.
- A media may not hold two external ids for the same provider.
- Deleting media soft-deletes (trash) by default; hard delete only after explicit purge.

## 7. Future Extensibility Points

- New contentType: add enum value + progress template + unit kind (data, not schema change).
- Cloud sync: the tracking/review/collection aggregates already carry updatedAt for last-write-wins
  policies; conflict resolution is designed at the aggregate boundary.
- Plugins: provider adapters are the first plugin seam (capability-based interface, `ARCHITECTURE.md §Provider`).
