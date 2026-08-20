/* MISSION-082 — Recap feature data layer. One query resolves a whole year's
   recap (headline totals + monthly chart + standouts) in a single round-trip. */

import { useQuery } from "@tanstack/react-query";
import { queryKeys, recap_year } from "@/api";
import type { GenreCount, RecapMedia, RecapTotals, YearRecap } from "@/api";

/** Resolve the year-in-review recap for one year. */
export function useRecapYearQuery(year: number) {
  return useQuery({
    queryKey: queryKeys.recap.year(year),
    queryFn: () => recap_year({ year }),
    placeholderData: (prev) => prev,
  });
}

export type { GenreCount, RecapMedia, RecapTotals, YearRecap };
