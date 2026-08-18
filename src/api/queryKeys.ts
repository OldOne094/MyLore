/* MISSION-035 — Typed query-key factory. Keys are readonly tuples built in
   exactly one place so cache entries can only collide by intent, and scope
   invalidation with the `*s`/list/detail fan-out pattern. Domains mirror upcoming
   commands; consumers arrive with later missions. */

import type { CsvMapping } from "@/api";

export interface MediaListFilters {
  content_type?: string | null;
  format?: string | null;
  pub_status?: string | null;
  genre?: string | null;
  tag?: string | null;
  year?: number | null;
  favorite?: boolean | null;
  search?: string | null;
  sort?: string | null;
  ascending?: boolean | null;
  limit?: number | null;
  offset?: number | null;
}

export const queryKeys = {
  system: {
    all: () => ["system"] as const,
    greeting: (name: string) => ["system", "greet", name] as const,
  },
  media: {
    all: () => ["media"] as const,
    lists: () => ["media", "list"] as const,
    list: (filters: MediaListFilters) => ["media", "list", filters] as const,
    facets: () => ["media", "facets"] as const,
    details: () => ["media", "detail"] as const,
    detail: (id: string) => ["media", "detail", id] as const,
    nodes: (id: string) => ["media", "nodes", id] as const,
    /** Personal tags linked to one media (MISSION-074). */
    tags: (id: string) => ["media", "tags", id] as const,
    /** Batch-resolved cover/banner assets (MISSION-062). `key` is the sorted,
        joined asset-id list so the same set dedupes to one cache entry. */
    assets: (key: string) => ["media", "assets", key] as const,
  },
  dashboard: {
    all: () => ["dashboard"] as const,
    summary: () => ["dashboard", "summary"] as const,
  },
  tracking: {
    all: () => ["tracking"] as const,
    detail: (mediaId: string) => ["tracking", "detail", mediaId] as const,
  },
  review: {
    all: () => ["review"] as const,
    forMedia: (mediaId: string) => ["review", "media", mediaId] as const,
  },
  collection: {
    all: () => ["collection"] as const,
    lists: () => ["collection", "list"] as const,
    detail: (collectionId: number) => ["collection", "detail", collectionId] as const,
  },
  stats: {
    all: () => ["stats"] as const,
    summary: (mediaId: number) => ["stats", "summary", mediaId] as const,
  },
  search: {
    all: () => ["search"] as const,
    local: (query: string) => ["search", "local", query] as const,
    external: (query: string, content_type: string | null) =>
      ["search", "external", query, content_type] as const,
  },
  import: {
    all: () => ["import"] as const,
    csvHeaders: (source: string, delimiter: string) =>
      ["import", "csv", "headers", source, delimiter] as const,
    /** Sniffed file kind (MISSION-072): `json`/`csv`/`anilist`/`goodreads`/
        `storygraph`, keyed by source so re-picking a file re-detects. */
    detect: (source: string) => ["import", "detect", source] as const,
    /** Per-file import preview (MISSION-069): keyed by kind + source + the
        effective mapping so any mapping change re-analyzes the file. */
    preview: (kind: string, source: string, mapping: CsvMapping | null) =>
      ["import", "preview", kind, source, mapping] as const,
  },
  trash: {
    all: () => ["trash"] as const,
    lists: () => ["trash", "list"] as const,
    list: () => ["trash", "list"] as const,
  },
  settings: {
    all: () => ["settings"] as const,
    preferences: () => ["settings", "preferences"] as const,
    /** Provider settings rows (MISSION-063). */
    providers: () => ["settings", "providers"] as const,
  },
  task: {
    all: () => ["task"] as const,
    detail: (taskId: string) => ["task", "detail", taskId] as const,
  },
} as const;
