/* MISSION-038 — Library feature data layer. Maps the user-facing input shape
   to the flat IPC arg shape (empty optional fields become null) and exposes a
   typed mutation that invalidates every library list on success. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { media_create, media_list } from "@/api";
import { queryKeys } from "@/api";
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
}

export interface MediaListArgs {
  content_type: string | null;
  pub_status: string | null;
  genre: string | null;
  tag: string | null;
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
  pub_status: null,
  genre: null,
  tag: null,
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
export function useMediaListQuery() {
  return useQuery({
    queryKey: queryKeys.media.lists(),
    queryFn: () => media_list(MEDIA_LIST_DEFAULT_ARGS),
  });
}
