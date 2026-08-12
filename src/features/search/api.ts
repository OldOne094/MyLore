/* MISSION-043 — Search feature data layer. Local full-text search over the
   library (FTS5 backend since MISSION-018). The hook is keyed under the
   `search.local` fan-out so repeated queries hit the cache. */

import { useQuery } from "@tanstack/react-query";
import { media_search } from "@/api";
import { queryKeys } from "@/api";

/** Search the local library by full-text query (MISSION-043). */
export function useMediaSearchQuery(query: string) {
  const trimmed = query.trim();
  return useQuery({
    queryKey: queryKeys.search.local(trimmed),
    queryFn: () => media_search({ query: trimmed }),
    enabled: trimmed.length > 0,
  });
}
