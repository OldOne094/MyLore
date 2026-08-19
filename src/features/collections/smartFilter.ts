/* MISSION-077 — Bridge between the library's filter/sort state and the
   SmartFilter shape the collection commands expect. The smart collection is a
   saved snapshot of exactly what the library is showing, so converting is a
   mechanical field copy. */

import type { SmartFilter } from "@/api";
import type { LibraryFilters, LibrarySort } from "@/features/library/filters";

/** A smart filter with every facet cleared — matches everything. */
export const EMPTY_SMART_FILTER: SmartFilter = {
  content_type: null,
  format: null,
  pub_status: null,
  genre: null,
  tag: null,
  year: null,
  favorite: null,
  sort: null,
  ascending: null,
};

export function toSmartFilter(filters: LibraryFilters, sort: LibrarySort): SmartFilter {
  return {
    content_type: filters.content_type,
    format: filters.format,
    pub_status: filters.pub_status,
    genre: filters.genre,
    tag: filters.tag,
    year: filters.year,
    favorite: filters.favorite,
    sort: sort.field,
    ascending: sort.ascending,
  };
}
