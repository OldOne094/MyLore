# MyLore — UX Research & Design Directions

> Phase 0 · August 2026
> Input: `RESEARCH.md` (competitor findings). Output feeds `DESIGN_SYSTEM.md` and the UI epics.

---

## 1. Audience & ergonomics

Desktop-first (Windows primary), single user, daily use for years. Design goals: fast, calm,
low-friction, keyboard-usable, dense-but-readable, RTL-capable, dark/light. Not "showy" —
**excellent daily workflow** (spec §80).

## 2. Navigation patterns (from research)

- **Left rail** for primary sections: Dashboard, Library, Discover, Collections, Import, Settings
  (persistent; collapsible). Universal in desktop media apps and file managers.
- **Status tabs within Library** (Planned / Reading / Completed / On-Hold / Dropped) — borrowed
  from MAL/AniList; implemented as filters, not separate pages, so they compose with
  type/genre/tag filters.
- **Command palette** (Ctrl/Cmd+K) for everything: search, add, mark complete, status change,
  navigate, settings (spec §52). This is the keyboard-first spine.
- **Context menus** on every list item/card (mark done, change status, add to list, edit, delete).
- Secondary details pane (master–detail) so progress capture never leaves the list (spec §33).

## 3. Page patterns

### 3.1 Dashboard
Customizable widget grid: Continue Reading/Watching (resume position), Recently Added,
Recently Completed, Quick Actions, Favorites, mini-stats. Empty-by-default, calm, no clutter
(REQ-DASH-001). Widgets toggleable in settings; not draggable in MVP (avoid complexity).

### 3.2 Library
Three densities: **Grid** (cards, progress overlay + quick buttons), **List** (rows, denser
metadata), **Compact list** (power-user). Toolbar: view switcher, sort (title/rating/progress/
date added/last updated/release), filter panel (type, format, status, genre, tag, year range,
favorite), group-by (status/type/year). Virtualized; debounced search box. Bulk-select mode
with action bar (status, add to list, tag, delete, export).

### 3.3 Media Detail
Hero: cover + titles (main + original/alt) + primary actions (start/resume, mark complete,
rating, favorite, add-to-list menu). Tabs: **Overview** (synopsis, meta facts), **Tracked**
(progress tree: seasons→episodes / volumes→chapters with per-node state; bulk-mark ranges),
**My Review** (rating, review, notes, spoiler toggle, personal tags), **Details** (genres, tags,
people, external sources + links), **Related**. Metadata (grey header block) and personal info
(own tab) visually separated (spec §32).

### 3.4 Quick capture
Global hotkey opens a compact popover: type-ahead → pick media → mark chapter/episode done or
set pages/episodes. One keystroke from anywhere (REQ-TRACK-005).

### 3.5 Discover & Search
Search modal: **Local results** first, then **External results** grouped by provider, each
external hit tagged "In library"/"Duplicate candidate" (identity check). One-click import →
preview → done. Discover page (optional): provider browsing, seasonal anime charts.

### 3.6 Collections / Lists
Sidebar group of collections + smart lists; smart list = saved filter (query builder later).
Drag/drop membership; bulk add.

### 3.7 Calendar
Month grid + list: air/release dates from media nodes (local, provider-independent), reading/
watching activity, completed items. Click → details.

### 3.8 Statistics
Cards + simple charts (bars/lines, no chart library bloat — hand-drawn SVG or a tiny lib):
counts, chapters/episodes/pages, hours, completion rate, avg rating, monthly activity, genre
distribution. Every number computed from local data only (REQ-STAT-001).

### 3.9 Import / Export / Backups / Settings
Wizard for import (choose source → parse → **preview with per-item outcomes** → run → report).
Export dialog (JSON/CSV/Markdown). Backups page (manual backup, list w/ restore + validation,
automatic schedule + rotation). Settings organized in tabs (General, Appearance, Tracking,
Providers, Notifications, Backup, Advanced/Logs).

### 3.10 Book / novel / web-novel specifics (from Goodreads, StoryGraph, NovelUpdates)
- **Chapter list as a tracking tree** for novels/web novels: rows with per-chapter read state and
  "my status" markers, bulk-mark ranges, one-tap "next chapter" (NovelUpdates' reading list).
- **Normal vs Manual mode:** explicit per-media toggle — auto-mark released nodes vs user sets the
  current chapter (NovelUpdates). Defaults to Manual; Normal only for auto-tracked WN/LN.
- **Mood / pace / content-warning badges** on the detail page (StoryGraph): surfaced from provider
  tags; content warnings offered as acknowledged-with-timestamp metadata, never forced.
- **DNF-with-progress:** "dropped" status carries the %/chapter where the user stopped
  (StoryGraph); expose it in detail + stats — our dropped status + `lastPosition` already cover it.
- **Currently-reading shelf:** in_progress as a first-class library filter (Goodreads).
- **Reading recap stats:** pages/chapters per month, mood/pace/format trends (StoryGraph) — a
  cheap, local-data-only addition to the stats page (REQ-STAT-001).

## 4. State & feedback patterns (spec §28, REQ-UX-005)

- **Loading:** skeletons matching final layout; never spinners for cards.
- **Empty states:** instructive + action button ("Add your first manga", "Import from AniList").
- **Errors:** inline + toast for transient; retry affordances; never dead screens.
- **Progress:** unobtrusive progress bars on covers; checkmarks for completed nodes; keyboard-visible.
- **Confirmation:** only for destructive/bulk/merge; reversible where possible (trash/undo) — fewer dialogs (P8).
- **Undo:** toast with Undo for deletes/status flips where cheap; trash for hard deletes/merges.

## 5. Keyboard & accessibility

- Full mnemonic coverage: `G D`/`G L` style, arrows in lists, `Enter` open, `Space` toggle, `Esc` close,
  `1-9` rate, `C` complete, `N` next chapter/episode. Command palette lists shortcuts.
- Focus visible in both themes; logical tab order; ARIA labels; `prefers-reduced-motion`.
- RTL: mirrored rail/back/forwards, numeric + date handling per locale (REQ-UX-003).

## 6. Design directions (spec §96)

### Design A — "Quiet Library" (RECOMMENDED for MVP)
Minimal, calm, editorial. Light gray surface, one accent, big typography, generous whitespace,
cards with quiet covers, progress as thin bars. Low cognitive load, daily-use comfort, easy to
keep polished.

### Design B — "Media Dashboard"
Media-server look (Plex/Jellyfin flavor): dramatic hero banners, gradient overlays, denser
poster walls, cinematic dark default. Higher visual energy; riskier to keep tasteful; heavier art.

### Design C — "Dense Power-User"
Table-heavy, information-dense rows, multi-sort, always-visible filters, tiny controls, status
chips everywhere. Maximizes throughput; harder for discovery & newcomers; risks feeling cramped.

**Comparison**

| Criterion | A | B | C |
|---|---|---|---|
| Daily-use comfort | ★★★★★ | ★★★★ | ★★★ |
| Information density | ★★★ | ★★★ | ★★★★★ |
| RTL/A11y cleanliness | ★★★★★ | ★★★★ | ★★★ |
| Effort to ship polished | Low | High | Medium |
| Long-term maintenance | Low | Medium | Medium |

**Decision:** base UI on **Design A**, add Design C affordances (keyboard, filters, bulk, compact
view) as progressive power features. The design system tokens keep B's "cinematic" touches
(rich covers, subtle gradients) as optional theming, not the default (see `DECISIONS.md` ADR-009).
