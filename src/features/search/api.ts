/* MISSION-043 — Search feature data layer. Local full-text search over the
   library (FTS5 backend since MISSION-018). The hook is keyed under the
   `search.local` fan-out so repeated queries hit the cache. */

import { useQuery } from "@tanstack/react-query";
import { media_search } from "@/api";
import { queryKeys } from "@/api";

/** Search the local library by full-text query (MISSION-043). */
export function useMediaSearchQuery(query: string, contentType?: string | null) {
  const trimmed = query.trim();
  const ct = contentType?.trim() || null;
  return useQuery({
    queryKey: queryKeys.search.local(trimmed, ct ?? undefined),
    queryFn: () => media_search({ query: trimmed, contentType: ct }),
    enabled: trimmed.length > 0,
    // Keep the previous results on screen while a new query is in flight so
    // type-ahead doesn't flash empty between keystrokes (MISSION-094).
    placeholderData: (previous) => previous,
  });
}
