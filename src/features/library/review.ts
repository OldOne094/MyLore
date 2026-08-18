/* MISSION-074 — Per-media review & notes. Reads the review row for the detail
   page's Review tab, saves it (validating the server-side invariants), clears
   it, and manages the media's personal tags. The save response is seeded into
   the review cache so the tab reflects the write immediately. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  media_add_tag,
  media_remove_tag,
  media_tags,
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
}

/** Read the review row for one media (`null` when unreviewed). */
export function useReviewQuery(mediaId: string) {
  return useQuery({
    queryKey: queryKeys.review.forMedia(mediaId),
    queryFn: () => review_get({ media_id: mediaId }),
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
      }),
    onSuccess: (view) => {
      queryClient.setQueryData(queryKeys.review.forMedia(view.media_id), view);
      void queryClient.invalidateQueries({ queryKey: queryKeys.media.details() });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() });
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
