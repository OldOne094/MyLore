/* MISSION-089 — Merge data layer. Planning is a read; applying folds the
   duplicate into the survivor and parks an undo image in the trash. */

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { merge_apply, merge_plan, type MergePreview, type MergeResult } from "@/api";
import { queryKeys } from "@/api";

/** Preview what merging the duplicate into the survivor would change. */
export function useMergePlan() {
  return useMutation({
    mutationFn: (input: { survivorId: string; duplicateId: string }): Promise<MergePreview> =>
      merge_plan({ survivor_id: input.survivorId, duplicate_id: input.duplicateId }),
  });
}

/** Apply a merge; the trash entry in the result is the undo handle. */
export function useMergeApply() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (input: { survivorId: string; duplicateId: string }): Promise<MergeResult> =>
      merge_apply({ survivor_id: input.survivorId, duplicate_id: input.duplicateId }),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: queryKeys.media.all() });
      void client.invalidateQueries({ queryKey: queryKeys.trash.all() });
    },
  });
}

export type { MergePreview, MergeResult };
