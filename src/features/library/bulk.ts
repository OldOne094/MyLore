/* MISSION-045 — Library bulk actions. Wraps the bulk IPC commands behind typed
   hooks: set tracking status, add a personal tag, soft-delete to trash (with a
   group undo via the trash ids), and add to a collection. Every success
   invalidates the affected cache fan-outs. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  collection_bulk_add,
  collection_list,
  media_bulk_add_tag,
  media_bulk_delete,
  tracking_bulk_set_status,
} from "@/api";
import { queryKeys } from "@/api";

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
    mutationFn: ({ ids, core_status }: { ids: string[]; core_status: string }) =>
      tracking_bulk_set_status({ ids, core_status }),
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
    mutationFn: ({ ids, tag }: { ids: string[]; tag: string }) => media_bulk_add_tag({ ids, tag }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.details() }),
      ]);
    },
  });
}

/** Soft-delete many media; resolves with a trash id per media (group undo). */
export function useBulkDelete() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => media_bulk_delete({ ids }),
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
    mutationFn: ({ collection_id, media_ids }: { collection_id: string; media_ids: string[] }) =>
      collection_bulk_add({ collection_id, media_ids }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.collection.all() });
    },
  });
}
