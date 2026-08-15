/* MISSION-059 — Discover feature data layer. External (provider) search over
   the coordinator, keyed under `search.external` so repeated queries hit the
   cache. */

import { useQuery } from "@tanstack/react-query";
import { search_external } from "@/api";
import { queryKeys } from "@/api";

/** Search providers by query (optionally narrowed to one content type). */
export function useDiscoverSearchQuery(query: string, content_type: string | null) {
  const trimmed = query.trim();
  return useQuery({
    queryKey: queryKeys.search.external(trimmed, content_type),
    queryFn: () => search_external({ query: trimmed, content_type }),
    enabled: trimmed.length > 0,
    staleTime: 60_000,
  });
}
