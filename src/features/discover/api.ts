/* MISSION-059 — Discover feature data layer. External (provider) search over
   the coordinator, keyed under `search.external` so repeated queries hit the
   cache. MISSION-060 adds the import-from-provider mutation, which resolves to
   the media owning the title (new or pre-existing). */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { import_provider, search_external } from "@/api";
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

/** Import one provider hit (provider + provider_id). On success the library,
 *  dashboard and facets are refreshed so the new title shows up everywhere. */
export function useImportProvider() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { provider: string; provider_id: string }) => import_provider(input),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.facets() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.details() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() }),
      ]);
    },
  });
}
