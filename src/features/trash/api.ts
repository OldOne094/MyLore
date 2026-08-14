/* MISSION-044 — Trash feature data layer. Delete is a soft delete: media_delete
   stores a before-image in trash and cascades the row away; undo toasts and the
   trash page call trash_restore / trash_purge. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { media_delete, trash_list, trash_purge, trash_restore } from "@/api";
import { queryKeys } from "@/api";

/** A trashed item as surfaced by `trash_list`. */
export interface TrashItem {
  id: string;
  kind: string;
  title: string;
  deleted_at: string;
}

/** Read the active trash list. */
export function useTrashListQuery() {
  return useQuery({
    queryKey: queryKeys.trash.list(),
    queryFn: () => trash_list(),
  });
}

/** Soft-delete a media; resolves with the trash id for undo. Invalidates every
 *  library list + detail so the row disappears everywhere at once. */
export function useDeleteMedia() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => media_delete({ id }),
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

/** Restore a trashed aggregate; invalidates trash + library. */
export function useRestoreTrashItem() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => trash_restore({ id }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.trash.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.details() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() }),
      ]);
    },
  });
}

/** Permanently forget a trash entry. */
export function usePurgeTrashItem() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => trash_purge({ id }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.trash.lists() });
    },
  });
}
