/* MISSION-048 — Per-media tracking state. Reads the tracking row for the
   detail page's Tracking tab and applies status transitions through the
   server-side status engine (which also runs the auto-complete rule on
   progress writes, so this cache is invalidated from `useNodeProgress`). */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { tracking_get, tracking_set_status } from "@/api";
import { queryKeys } from "@/api";

/** Read the tracking row for one media (`null` when untracked). */
export function useTrackingQuery(mediaId: string) {
  return useQuery({
    queryKey: queryKeys.tracking.detail(mediaId),
    queryFn: () => tracking_get({ media_id: mediaId }),
  });
}

/** Apply a status transition for one media; seeds the detail cache with the
    server-returned row so the picker reflects the transition immediately. */
export function useSetStatus() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ media_id, core_status }: { media_id: string; core_status: string }) =>
      tracking_set_status({ media_id, core_status }),
    onSuccess: (view) => {
      queryClient.setQueryData(queryKeys.tracking.detail(view.media_id), view);
    },
  });
}
