/* MISSION-080 — Stats feature data layer. A single query resolves the
   whole-library overview (counts, distributions, consumption) for the Stats
   page in one round-trip. */

import { useQuery } from "@tanstack/react-query";
import { stats_summary } from "@/api";
import { queryKeys } from "@/api";
import type { StatCount, StatsView } from "@/api";

/** Resolve the library statistics overview. */
export function useStatsSummaryQuery() {
  return useQuery({
    queryKey: queryKeys.stats.summary(),
    queryFn: () => stats_summary(),
  });
}

export type { StatCount, StatsView };
