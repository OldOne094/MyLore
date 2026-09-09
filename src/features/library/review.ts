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
  review_list,
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
    queryFn: () => review_get({ mediaId: mediaId }),
    enabled,
  });
}

/** Every review in the library with its media's display fields, newest first
    (MISSION-144) — feeds the aggregate Reviews hub. */
export function useReviewsQuery() {
  return useQuery({
    queryKey: queryKeys.review.list(),
    queryFn: () => review_list(),
  });
}

/** Save (create or update) a media's review; seeds the review cache. */
export function useSaveReview() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: SaveReviewInput) =>
      review_save({
        mediaId: input.media_id,
        rating: input.rating,
        review: input.review,
        shortReview: input.short_review,
        notes: input.notes,
        favorite: input.favorite,
        isSpoiler: input.is_spoiler,
        moods: input.moods,
        pace: input.pace,
        contentWarnings: input.content_warnings,
      }),
    onSuccess: (view) => {
      queryClient.setQueryData(queryKeys.review.forMedia(view.media_id), view);
      void queryClient.invalidateQueries({ queryKey: queryKeys.review.list() });
      void queryClient.invalidateQueries({ queryKey: queryKeys.media.details() });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() });
    },
  });
}

/** Acknowledge a media's current content-warning set; seeds the cache. */
export function useAcknowledgeWarnings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (mediaId: string) => review_acknowledge_warnings({ mediaId: mediaId }),
    onSuccess: (view) => {
      queryClient.setQueryData(queryKeys.review.forMedia(view.media_id), view);
      void queryClient.invalidateQueries({ queryKey: queryKeys.review.list() });
    },
  });
}

/** Delete a media's review; seeds an empty cache entry. */
export function useDeleteReview() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (mediaId: string) => review_delete({ mediaId: mediaId }),
    onSuccess: (_void, mediaId) => {
      queryClient.setQueryData(queryKeys.review.forMedia(mediaId), null);
      void queryClient.invalidateQueries({ queryKey: queryKeys.review.list() });
      void queryClient.invalidateQueries({ queryKey: queryKeys.media.details() });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() });
    },
  });
}

/** The personal tags linked to one media (MISSION-074). */
export function useMediaTagsQuery(mediaId: string) {
  return useQuery({
    queryKey: queryKeys.media.tags(mediaId),
    queryFn: () => media_tags({ mediaId: mediaId }),
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
    mutationFn: ({ mediaId, tag }: { mediaId: string; tag: string }) =>
      media_add_tag({ mediaId: mediaId, tag }),
    onSuccess: (tags, { mediaId }) => seedTags(queryClient, mediaId, tags),
  });
}

/** Remove a personal tag from one media; seeds the updated tag list. */
export function useRemoveMediaTag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ mediaId, tagId }: { mediaId: string; tagId: string }) =>
      media_remove_tag({ mediaId: mediaId, tagId: tagId }),
    onSuccess: (tags, { mediaId }) => seedTags(queryClient, mediaId, tags),
  });
}
