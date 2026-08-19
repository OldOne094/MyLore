/* MISSION-041 — Library filter/sort/group state. Serialized into the flat IPC
   args shape in `toMediaListArgs`. Every facet is nullable so an empty state
   equals "no filters" and mapping to the command stays mechanical. */

import type { BulkFilter } from "@/api";
import type { MediaListArgs } from "./api";
import type { LibraryGroupBy } from "./grouping";

export interface LibraryFilters {
  content_type: string | null;
  format: string | null;
  pub_status: string | null;
  genre: string | null;
  tag: string | null;
  year: number | null;
  favorite: boolean | null;
}

export const DEFAULT_FILTERS: LibraryFilters = {
  content_type: null,
  format: null,
  pub_status: null,
  genre: null,
  tag: null,
  year: null,
  favorite: null,
};

export type LibrarySortField = "title" | "created_at" | "updated_at" | "release_year";

export interface LibrarySort {
  field: LibrarySortField;
  ascending: boolean;
}

export const DEFAULT_SORT: LibrarySort = { field: "title", ascending: true };

export const SORT_FIELDS: LibrarySortField[] = [
  "title",
  "created_at",
  "updated_at",
  "release_year",
];

/** How many filter facets are active (for the toolbar badge). */
export function activeFilterCount(filters: LibraryFilters): number {
  let count = 0;
  for (const value of [
    filters.content_type,
    filters.format,
    filters.pub_status,
    filters.genre,
    filters.tag,
  ]) {
    if (value !== null) count += 1;
  }
  if (filters.year !== null) count += 1;
  if (filters.favorite !== null) count += 1;
  return count;
}

export function filtersToArgs(filters: LibraryFilters, sort: LibrarySort): MediaListArgs {
  return {
    content_type: filters.content_type,
    format: filters.format,
    pub_status: filters.pub_status,
    genre: filters.genre,
    tag: filters.tag,
    year: filters.year,
    favorite: filters.favorite,
    search: null,
    sort: sort.field,
    ascending: sort.ascending,
    limit: null,
    offset: null,
  };
}

/** Map active filters to the server-side bulk-op scope (MISSION-078). A falsy
    filter maps to `null`, keeping the explicit-ids path of the bulk commands. */
export function toBulkFilter(filter: LibraryFilters | null | undefined): BulkFilter | null {
  if (!filter) return null;
  return {
    content_type: filter.content_type,
    format: filter.format,
    pub_status: filter.pub_status,
    genre: filter.genre,
    tag: filter.tag,
    year: filter.year,
    favorite: filter.favorite,
  };
}

export type { LibraryGroupBy };
