/* MISSION-049 — Quick-capture progress. `useMarkNextUnit` advances a media one
   unit through `node_progress_next` and refreshes every cache that renders
   progress (grid cards, detail nodes, tracking, search results). `unreadUnits`
   flattens a content tree to the countable units still to be consumed, which
   drives the popover's "mark up to N" ranges. */

import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { node_progress_next, node_progress_range } from "@/api";
import { queryKeys } from "@/api";
import { useToast } from "@/components/ui";
import type { ContentNode } from "@/api";

/** Unit node kinds that count toward progress (mirrors the backend UNIT_KINDS). */
const UNIT_KINDS = new Set(["episode", "chapter", "node"]);

/** Consuming state per content type (mirrors the backend ProgressTemplate):
    episodes/movies are "watched", everything else is "read". */
export function consumingStateFor(contentType: string): string {
  if (contentType === "anime" || contentType === "tv" || contentType === "movie") return "watched";
  return "read";
}

/** Countable unit nodes still to be consumed, in display order. Skipped nodes
    remain candidates — the backend treats them as next-to-mark too. */
export function unreadUnits(nodes: ContentNode[], consumingState: string): ContentNode[] {
  const unread: ContentNode[] = [];
  const walk = (list: ContentNode[]) => {
    for (const node of list) {
      if (UNIT_KINDS.has(node.kind) && node.state !== consumingState) unread.push(node);
      walk(node.children);
    }
  };
  walk(nodes);
  return unread;
}

/** Short label for a countable node: "E4", "Ch7", "#12" (mirrors backend). */
export function nodeUnitLabel(node: ContentNode): string {
  const raw = node.number?.trim() || (node.position > 0 ? String(node.position) : "");
  if (node.kind === "episode") return raw ? `E${raw}` : "E";
  if (node.kind === "chapter") return raw ? `Ch${raw}` : "Ch";
  return raw ? `#${raw}` : "#";
}

/** Refetch every cache that renders progress after a progress write. */
export async function invalidateProgress(queryClient: QueryClient, mediaId?: string) {
  const jobs: Promise<void>[] = [
    queryClient.invalidateQueries({ queryKey: queryKeys.media.all() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.tracking.all() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.search.all() }),
  ];
  if (mediaId)
    jobs.push(queryClient.invalidateQueries({ queryKey: queryKeys.media.nodes(mediaId) }));
  await Promise.all(jobs);
}

/** Advance one unit. Resolves with the refreshed summary view, or `null` when
    there is nothing left to mark (callers surface the "all caught up" toast). */
export function useMarkNextUnit() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();
  return useMutation({
    mutationFn: (mediaId: string) => node_progress_next({ media_id: mediaId }),
    onSuccess: async (_view, mediaId) => invalidateProgress(queryClient, mediaId),
    onError: () => toast.error({ title: t("progress.setErrorToast") }),
  });
}

/** Mark every node between `fromId` and `toId` (in display order) with the
    consuming state, then refresh progress caches. */
export function useMarkRange(mediaId: string) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();
  return useMutation({
    mutationFn: ({ fromId, toId, state }: { fromId: string; toId: string; state: string }) =>
      node_progress_range({ media_id: mediaId, from_id: fromId, to_id: toId, node_state: state }),
    onSuccess: async () => invalidateProgress(queryClient, mediaId),
    onError: () => toast.error({ title: t("progress.rangeErrorToast") }),
  });
}
