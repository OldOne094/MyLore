/* MISSION-038 — Library feature data layer. Maps the user-facing input shape
   to the flat IPC arg shape (empty optional fields become null) and exposes a
   typed mutation that invalidates every library list on success. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  media_create,
  media_facets,
  media_get,
  media_list,
  media_nodes,
  node_progress_range,
  node_progress_set,
} from "@/api";
import { queryKeys } from "@/api";
import { useToast } from "@/components/ui";
import type { ContentNode, ProgressSummary } from "@/api";
import type { AddMediaInput } from "./types";

export interface MediaCreateArgs {
  title: string;
  content_type: string;
  format: string | null;
  pub_status: string | null;
  synopsis: string | null;
  release_year: number | null;
  language: string | null;
  country: string | null;
  pages: number | null;
  duration_min: number | null;
  ep_count: number | null;
  ch_count: number | null;
  genres: string[];
}

/** A slim row returned by `media_list`, backing the library grid. */
export interface MediaListItem {
  id: string;
  content_type: string;
  title: string;
  pub_status: string;
  release_year: number | null;
  cover_asset_id: string | null;
  updated_at: string;
  /** Derived progress summary driving the in-grid quick controls (MISSION-049). */
  progress: ProgressSummary;
}

/** A selectable facet value (`genre`/`tag` rows carry an id + display name). */
export interface MediaFacetOption {
  id: string;
  name: string;
}

/** Full aggregate returned by `media_get`, backing the detail page (MISSION-042). */
export interface MediaDetail {
  id: string;
  content_type: string;
  format: string | null;
  title_main: string;
  title_original: string | null;
  synopsis: string | null;
  pub_status: string;
  start_date: string | null;
  end_date: string | null;
  release_year: number | null;
  language: string | null;
  country: string | null;
  content_rating: string | null;
  pages: number | null;
  duration_min: number | null;
  ep_count: number | null;
  ch_count: number | null;
  cover_asset_id: string | null;
  banner_asset_id: string | null;
  provider: string | null;
  provider_url: string | null;
  metadata_refreshed_at: string | null;
  created_at: string;
  updated_at: string;
  alt_titles: { lang: string; title: string }[];
  people: string[];
  genres: string[];
  tags: string[];
  external_ids: { provider: string; ext_id: string; url: string | null }[];
  relations: { to_id: string; relation: string }[];
}

/** Distinct filter values present in the library (MISSION-041). */
export interface MediaFacets {
  formats: string[];
  genres: MediaFacetOption[];
  tags: MediaFacetOption[];
  years: number[];
}

export interface MediaListArgs {
  content_type: string | null;
  format: string | null;
  pub_status: string | null;
  genre: string | null;
  tag: string | null;
  year: number | null;
  favorite: boolean | null;
  search: string | null;
  sort: string | null;
  ascending: boolean | null;
  limit: number | null;
  offset: number | null;
}

/** Default library listing: everything, title ascending. */
export const MEDIA_LIST_DEFAULT_ARGS: MediaListArgs = {
  content_type: null,
  format: null,
  pub_status: null,
  genre: null,
  tag: null,
  year: null,
  favorite: null,
  search: null,
  sort: "title",
  ascending: true,
  limit: null,
  offset: null,
};

export function toMediaCreateArgs(input: AddMediaInput): MediaCreateArgs {
  return {
    title: input.title,
    content_type: input.contentType,
    format: input.format ?? null,
    pub_status: input.pubStatus ?? null,
    synopsis: input.synopsis ?? null,
    release_year: input.releaseYear ?? null,
    language: input.language ?? null,
    country: input.country ?? null,
    pages: input.pages ?? null,
    duration_min: input.durationMin ?? null,
    ep_count: input.epCount ?? null,
    ch_count: input.chCount ?? null,
    genres: input.genres,
  };
}

export function useAddMedia() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: AddMediaInput) => media_create(toMediaCreateArgs(input)),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() });
    },
  });
}

/** Read the library grid; keyed under the list fan-out so adds invalidate it. */
export function useMediaListQuery(args: MediaListArgs = MEDIA_LIST_DEFAULT_ARGS) {
  return useQuery({
    queryKey: queryKeys.media.list(args),
    queryFn: () => media_list(args),
  });
}

/** Distinct filter values present in the library (MISSION-041). */
export function useMediaFacetsQuery() {
  return useQuery({
    queryKey: queryKeys.media.facets(),
    queryFn: () => media_facets(),
  });
}

/** Read the full aggregate for one media (MISSION-042). */
export function useMediaDetailQuery(id: string) {
  return useQuery({
    queryKey: queryKeys.media.detail(id),
    queryFn: () => media_get({ id }),
  });
}

/** Read the content tree of one media (MISSION-046). */
export function useMediaNodesQuery(id: string) {
  return useQuery({
    queryKey: queryKeys.media.nodes(id),
    queryFn: () => media_nodes({ id }),
  });
}

/**
 * MISSION-047 — progress actions for the content tree.
 *
 * Both actions apply an optimistic state to the nodes cache immediately, roll
 * the cache back if the IPC write rejects, and — for ranges — reconcile the
 * cache with the server-returned affected ids after the write succeeds so the
 * exact boundary never drifts.
 */
export function useNodeProgress(mediaId: string) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();
  const key = queryKeys.media.nodes(mediaId);

  const commit = (nodeIds: string[], state: string | null) => {
    const previous = queryClient.getQueryData<ContentNode[]>(key) ?? [];
    queryClient.setQueryData(key, setNodeState(previous, new Set(nodeIds), state));
    return previous;
  };

  const markNode = async (nodeId: string, state: string) => {
    const previous = commit([nodeId], state);
    try {
      await node_progress_set({ node_id: nodeId, node_state: state });
      await queryClient.invalidateQueries({ queryKey: queryKeys.tracking.all() });
    } catch {
      queryClient.setQueryData(key, previous);
      toast.error({ title: t("progress.setErrorToast") });
    }
  };

  const markRange = async (fromId: string, toId: string, state: string, rangeIds: string[]) => {
    const previous = commit(rangeIds, state);
    try {
      const affected = await node_progress_range({
        media_id: mediaId,
        from_id: fromId,
        to_id: toId,
        node_state: state,
      });
      queryClient.setQueryData(key, setNodeState(previous, new Set(affected), state));
      await queryClient.invalidateQueries({ queryKey: queryKeys.tracking.all() });
    } catch {
      queryClient.setQueryData(key, previous);
      toast.error({ title: t("progress.rangeErrorToast") });
    }
  };

  return { markNode, markRange };
}

/** Immutably set `state` on every matching node id, recursing into children. */
function setNodeState(nodes: ContentNode[], ids: Set<string>, state: string | null): ContentNode[] {
  let changed = false;
  const next = nodes.map((node) => {
    const children = setNodeState(node.children, ids, state);
    if (ids.has(node.id) || children !== node.children) changed = true;
    return ids.has(node.id) ? { ...node, children, state } : { ...node, children };
  });
  return changed ? next : nodes;
}
