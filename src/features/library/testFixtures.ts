import type { ProgressSummary } from "@/api";
import type { MediaListItem } from "./api";

/* MISSION-049 — Shared test fixtures for library rows. `listItem` builds a
   valid `MediaListItem` (with the progress summary the backend now always
   includes) so library tests can override just the fields they care about. */

export const NO_PROGRESS: ProgressSummary = {
  percent: null,
  completed: 0,
  total: 0,
  next_label: null,
  next_node_id: null,
};

export function listItem(overrides: Partial<MediaListItem>): MediaListItem {
  return {
    id: "m-1",
    content_type: "novel",
    title: "Title",
    pub_status: "ongoing",
    release_year: 2024,
    cover_asset_id: null,
    updated_at: "2026-01-01T00:00:00Z",
    progress: NO_PROGRESS,
    ...overrides,
  };
}
