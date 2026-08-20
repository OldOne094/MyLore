/* MISSION-081 — Calendar feature data layer. One query resolves a whole month
   of air/release dates + activity in a single round-trip. */

import { useQuery } from "@tanstack/react-query";
import { calendar_month } from "@/api";
import { queryKeys } from "@/api";
import type { CalendarDay, CalendarItem, CalendarMonth } from "@/api";

/** Resolve one calendar month (air dates + activity bucketed per day). */
export function useCalendarMonthQuery(year: number, month: number) {
  return useQuery({
    queryKey: queryKeys.calendar.month(year, month),
    queryFn: () => calendar_month({ year, month }),
    placeholderData: (prev) => prev,
  });
}

export type { CalendarDay, CalendarItem, CalendarMonth };
