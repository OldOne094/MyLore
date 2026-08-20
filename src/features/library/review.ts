/* MISSION-074 — Per-media review & notes. Reads the review row for the detail
   page's Review tab, saves it (validating the server-side invariants), clears
   it, and manages the media's personal tags. MISSION-079 adds the
   mood/pace/content-warning metadata and the content-warning acknowledgment.
   The save/acknowledge responses are seeded into the review cache so the tab
   and the detail-page badges reflect the write immediately. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  media_add_tag,
  media_remove_tag,
  media_tags,
  review_acknowledge_warnings,
  review_delete,
  review_get,
  review_save,
  type MediaTagView,
} from "@/api";
import { queryKeys } from "@/api";

/** The full review payload the Review tab submits. */
export interface SaveReviewInput {
  media_id: string;
  rating: number | null;
  review: string | null;
  short_review: string | null;
  notes: string | null;
  favorite: boolean;
  is_spoiler: boolean;
  /** Canonical mood keys (MISSION-079). */
  moods: string[];
  pace: string | null;
  /** Canonical content-warning keys (MISSION-079). */
  content_warnings: string[];
}

/** Read the review row for one media (`null` when unreviewed). `enabled` lets
    callers defer the fetch until a media id is available (MISSION-079). */
export function useReviewQuery(mediaId: string, enabled = true) {
  return useQuery({
    queryKey: queryKeys.review.forMedia(mediaId),
    queryFn: () => review_get({ media_id: mediaId }),
    enabled,
  });
}

/** Save (create or update) a media's review; seeds the review cache. */
export function useSaveReview() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: SaveReviewInput) =>
      review_save({
        media_id: input.media_id,
        rating: input.rating,
        review: input.review,
        short_review: input.short_review,
        notes: input.notes,
        favorite: input.favorite,
        is_spoiler: input.is_spoiler,
        moods: input.moods,
        pace: input.pace,
        content_warnings: input.content_warnings,
      }),
    onSuccess: (view) => {
      queryClient.setQueryData(queryKeys.review.forMedia(view.media_id), view);
      void queryClient.invalidateQueries({ queryKey: queryKeys.media.details() });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() });
    },
  });
}

/** Acknowledge a media's current content-warning set; seeds the cache. */
export function useAcknowledgeWarnings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (media_id: string) => review_acknowledge_warnings({ media_id }),
    onSuccess: (view) => {
      queryClient.setQueryData(queryKeys.review.forMedia(view.media_id), view);
    },
  });
}

/** Delete a media's review; seeds an empty cache entry. */
export function useDeleteReview() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (media_id: string) => review_delete({ media_id }),
    onSuccess: (_void, mediaId) => {
      queryClient.setQueryData(queryKeys.review.forMedia(mediaId), null);
      void queryClient.invalidateQueries({ queryKey: queryKeys.media.details() });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() });
    },
  });
}

/** The personal tags linked to one media (MISSION-074). */
export function useMediaTagsQuery(mediaId: string) {
  return useQuery({
    queryKey: queryKeys.media.tags(mediaId),
    queryFn: () => media_tags({ media_id: mediaId }),
  });
}

function seedTags(
  queryClient: ReturnType<typeof useQueryClient>,
  mediaId: string,
  tags: MediaTagView[],
) {
  queryClient.setQueryData(queryKeys.media.tags(mediaId), tags);
  void queryClient.invalidateQueries({ queryKey: queryKeys.media.details() });
}

/** Add a personal tag to one media; seeds the updated tag list. */
export function useAddMediaTag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ media_id, tag }: { media_id: string; tag: string }) =>
      media_add_tag({ media_id, tag }),
    onSuccess: (tags, { media_id }) => seedTags(queryClient, media_id, tags),
  });
}

/** Remove a personal tag from one media; seeds the updated tag list. */
export function useRemoveMediaTag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ media_id, tag_id }: { media_id: string; tag_id: string }) =>
      media_remove_tag({ media_id, tag_id }),
    onSuccess: (tags, { media_id }) => seedTags(queryClient, media_id, tags),
  });
}
