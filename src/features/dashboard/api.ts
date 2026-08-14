/* MISSION-050 — Dashboard feature data layer. A single query resolves all
   widget lists in one round-trip so the home page renders with one loading
   state and stays calm when the library is empty. */

import { useQuery } from "@tanstack/react-query";
import { dashboard_summary } from "@/api";
import { queryKeys } from "@/api";
import type { DashboardSummary, MediaListItem } from "@/api";

/** Resolve the dashboard widget lists (continue, recently completed, added). */
export function useDashboardSummaryQuery(limit?: number) {
  return useQuery({
    queryKey: queryKeys.dashboard.summary(),
    queryFn: () => dashboard_summary({ limit: limit ?? null }),
  });
}

export type { DashboardSummary, MediaListItem };
