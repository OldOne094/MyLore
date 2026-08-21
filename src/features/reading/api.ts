/* MISSION-083 — Reading recap data layer. One query resolves the whole recap:
   pages/chapters per month for a chosen year, the year totals, and the
   all-time mood / pace / format taste distributions. */

import { useQuery } from "@tanstack/react-query";
import { queryKeys, reading_recap } from "@/api";
import type { MonthReading, ReadingRecap, ReadingTotals, StatCount } from "@/api";

/** Resolve the reading recap for one year. */
export function useReadingRecapQuery(year: number) {
  return useQuery({
    queryKey: queryKeys.readingRecap.year(year),
    queryFn: () => reading_recap({ year }),
    placeholderData: (prev) => prev,
  });
}

export type { MonthReading, ReadingRecap, ReadingTotals, StatCount };
