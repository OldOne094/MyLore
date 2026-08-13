/* MISSION-035 — Typed query-key factory. Keys are readonly tuples built in
   exactly one place so cache entries can only collide by intent, and scope
   invalidation with the `*s`/list/detail fan-out pattern. Domains mirror upcoming
   commands; consumers arrive with later missions. */

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
  },
  tracking: {
    all: () => ["tracking"] as const,
    detail: (mediaId: number) => ["tracking", "detail", mediaId] as const,
    node: (mediaId: number, nodeId: number) => ["tracking", "node", mediaId, nodeId] as const,
  },
  review: {
    all: () => ["review"] as const,
    forMedia: (mediaId: number) => ["review", "media", mediaId] as const,
    detail: (reviewId: number) => ["review", "detail", reviewId] as const,
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
  },
  trash: {
    all: () => ["trash"] as const,
    lists: () => ["trash", "list"] as const,
    list: () => ["trash", "list"] as const,
  },
  settings: {
    all: () => ["settings"] as const,
    preferences: () => ["settings", "preferences"] as const,
  },
  task: {
    all: () => ["task"] as const,
    detail: (taskId: string) => ["task", "detail", taskId] as const,
  },
} as const;
