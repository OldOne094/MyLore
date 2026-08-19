/* MISSION-045 — Library bulk actions. Wraps the bulk IPC commands behind typed
   hooks: set tracking status, add a personal tag, soft-delete to trash (with a
   group undo via the trash ids), and add to a collection. Every success
   invalidates the affected cache fan-outs. MISSION-078 adds an optional
   `filter` scope so an action can apply to the whole filtered selection
   (resolved server-side) and resolves with a per-item `BulkResult` summary. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  collection_bulk_add,
  collection_list,
  media_bulk_add_tag,
  media_bulk_delete,
  tracking_bulk_set_status,
} from "@/api";
import { queryKeys } from "@/api";
import type { BulkDeleteResult, BulkResult } from "@/api";
import type { LibraryFilters } from "./filters";
import { toBulkFilter } from "./filters";

/** Which media an action touches. Either the explicit selection (`ids`) or,
    when `filter` is set, the whole filtered selection — the backend resolves
    the ids server-side and ignores `ids`. */
export interface BulkScope {
  ids: string[];
  filter?: LibraryFilters | null;
}

/** Read the collections available for the "add to list" action. */
export function useCollectionListQuery() {
  return useQuery({
    queryKey: queryKeys.collection.lists(),
    queryFn: () => collection_list(),
  });
}

/** Set the tracking status for many media at once. */
export function useBulkSetStatus() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      ids,
      filter,
      core_status,
    }: BulkScope & { core_status: string }): Promise<BulkResult> =>
      tracking_bulk_set_status({ ids, core_status, filter: toBulkFilter(filter) }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.details() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.tracking.all() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() }),
      ]);
    },
  });
}

/** Add a personal tag to many media at once. */
export function useBulkAddTag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ ids, filter, tag }: BulkScope & { tag: string }): Promise<BulkResult> =>
      media_bulk_add_tag({ ids, tag, filter: toBulkFilter(filter) }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.details() }),
      ]);
    },
  });
}

/** Soft-delete many media; resolves with a trash id per deleted media (group
    undo restores exactly what was removed). */
export function useBulkDelete() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ ids, filter }: BulkScope): Promise<BulkDeleteResult> =>
      media_bulk_delete({ ids, filter: toBulkFilter(filter) }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.details() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.trash.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() }),
      ]);
    },
  });
}

/** Add many media to one collection. */
export function useBulkAddToCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      collection_id,
      ids,
      filter,
    }: BulkScope & { collection_id: string }): Promise<BulkResult> =>
      collection_bulk_add({ collection_id, media_ids: ids, filter: toBulkFilter(filter) }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.collection.all() });
    },
  });
}
