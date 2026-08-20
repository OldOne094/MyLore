import { ChevronLeft, ChevronRight, RefreshCcw, Calendar } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useCalendarMonthQuery, type CalendarDay, type CalendarItem } from "./api";

/* MISSION-081 — Calendar page (REQ-CAL-001). A month grid of content-node
   air/release dates plus the user activity trail, with a day list beside it.
   Hand-rolled (no date library) and RTL-safe via logical properties; each day
   cell carries two small dots — one for air dates, one for activity. */

const WEEKDAYS = ["day0", "day1", "day2", "day3", "day4", "day5", "day6"] as const;

function monthCells(year: number, month: number): (number | null)[] {
  const leading = new Date(year, month - 1, 1).getDay();
  const count = new Date(year, month, 0).getDate();
  return [
    ...Array.from({ length: leading }, () => null),
    ...Array.from({ length: count }, (_, index) => index + 1),
  ];
}

function dateKey(year: number, month: number, day: number): string {
  return `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

function CalendarSkeleton() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("nav.calendar")} role="status" className="px-5 py-5">
      <div className="flex items-center justify-between">
        <Skeleton className="h-9 w-9" />
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-9 w-9" />
      </div>
      <div className="mt-3 grid grid-cols-7 gap-1">
        {Array.from({ length: 35 }, (_, index) => (
          <Skeleton key={index} className="h-14" />
        ))}
      </div>
    </section>
  );
}

function EventRow({
  item,
  kindLabel,
  time,
  variant,
}: {
  item: CalendarItem;
  kindLabel: string;
  time: string | null;
  variant: "air" | "activity";
}) {
  const { t } = useTranslation();
  const title = item.title || t("calendar.deletedMedia");
  const typeLabel = item.content_type ? t(`contentType.${item.content_type}`) : null;
  return (
    <li className="flex flex-col gap-0.5">
      <div className="flex items-center gap-2">
        <span
          className={cn(
            "inline-flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium",
            variant === "air"
              ? "border-accent/30 bg-accent/10 text-accent"
              : "border-border-strong bg-bg-raised text-text-secondary",
          )}
        >
          {kindLabel}
        </span>
        <span className="truncate text-sm text-text-primary">
          {item.media_id ? (
            <Link
              className="transition-colors duration-150 ease-out hover:text-accent"
              to={`/library/${item.media_id}`}
            >
              {title}
            </Link>
          ) : (
            title
          )}
        </span>
      </div>
      {(typeLabel || time) && (
        <div className="flex items-center gap-2 ps-1">
          {typeLabel && <span className="text-xs text-text-tertiary">{typeLabel}</span>}
          {time && <span className="text-xs tabular-nums text-text-tertiary">{time}</span>}
        </div>
      )}
    </li>
  );
}

export function CalendarPage() {
  const { t, i18n } = useTranslation();
  const [cursor, setCursor] = useState(() => {
    const now = new Date();
    return { year: now.getFullYear(), month: now.getMonth() + 1 };
  });
  const [selectedDay, setSelectedDay] = useState<number | null>(null);

  const now = new Date();
  const isCurrentMonth = now.getFullYear() === cursor.year && now.getMonth() + 1 === cursor.month;

  const [prevCursor, setPrevCursor] = useState(cursor);
  if (prevCursor.year !== cursor.year || prevCursor.month !== cursor.month) {
    setPrevCursor(cursor);
    setSelectedDay(isCurrentMonth ? now.getDate() : 1);
  }

  const { data, isLoading, isError, refetch } = useCalendarMonthQuery(cursor.year, cursor.month);
  const monthReady = data?.year === cursor.year && data?.month === cursor.month;
  const byDate = new Map((monthReady ? data.days : []).map((day) => [day.date, day]));

  const selectedKey = selectedDay != null ? dateKey(cursor.year, cursor.month, selectedDay) : null;
  const dayInfo = selectedKey ? byDate.get(selectedKey) : undefined;

  const today = new Date();
  const todayKey = dateKey(today.getFullYear(), today.getMonth() + 1, today.getDate());
  const isRtl = i18n.dir() === "rtl";

  const monthLabel = new Date(cursor.year, cursor.month - 1, 1).toLocaleDateString(i18n.language, {
    month: "long",
    year: "numeric",
  });

  const shiftMonth = (delta: number) =>
    setCursor(({ year, month }) => {
      const next = new Date(year, month - 1 + delta, 1);
      return { year: next.getFullYear(), month: next.getMonth() + 1 };
    });

  if (isError) {
    return (
      <EmptyState
        icon={Calendar}
        title={t("calendar.errorTitle")}
        hint={t("library.errorHint")}
        action={
          <Button variant="secondary" onClick={() => void refetch()}>
            <RefreshCcw size={16} aria-hidden="true" />
            {t("library.retry")}
          </Button>
        }
      />
    );
  }

  if (isLoading || !monthReady) return <CalendarSkeleton />;

  const cells = monthCells(cursor.year, cursor.month);

  return (
    <section aria-label={t("nav.calendar")} className="px-5 py-5">
      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
        <div>
          <div className="flex items-center justify-between">
            <Button
              variant="ghost"
              size="sm"
              aria-label={t("calendar.prevMonth")}
              onClick={() => shiftMonth(-1)}
            >
              {isRtl ? (
                <ChevronRight size={18} aria-hidden="true" />
              ) : (
                <ChevronLeft size={18} aria-hidden="true" />
              )}
            </Button>
            <h2 className="text-sm font-semibold text-text-primary">{monthLabel}</h2>
            <Button
              variant="ghost"
              size="sm"
              aria-label={t("calendar.nextMonth")}
              onClick={() => shiftMonth(1)}
            >
              {isRtl ? (
                <ChevronLeft size={18} aria-hidden="true" />
              ) : (
                <ChevronRight size={18} aria-hidden="true" />
              )}
            </Button>
          </div>

          <div className="mt-3 grid grid-cols-7 gap-1">
            {WEEKDAYS.map((key) => (
              <div
                key={key}
                className="px-1 pb-1 text-center text-[11px] font-medium uppercase tracking-wide text-text-tertiary"
              >
                {t(`calendar.${key}`)}
              </div>
            ))}
            {cells.map((day, index) => {
              if (day === null) return <div key={`blank-${index}`} />;
              const key = dateKey(cursor.year, cursor.month, day);
              const info = byDate.get(key) as CalendarDay | undefined;
              const isSelected = selectedDay === day;
              return (
                <button
                  key={key}
                  type="button"
                  onClick={() => setSelectedDay(day)}
                  aria-pressed={isSelected}
                  className={cn(
                    "flex h-14 flex-col items-center justify-start gap-1 rounded-sm border p-1 text-sm transition-colors duration-150 ease-out hover:bg-bg-hover",
                    isSelected
                      ? "border-accent/40 bg-accent-soft text-accent"
                      : "border-border-subtle bg-bg-surface text-text-primary",
                    key === todayKey && !isSelected && "ring-1 ring-inset ring-accent",
                  )}
                >
                  <span className={cn("tabular-nums", key === todayKey && "font-semibold")}>
                    {day}
                  </span>
                  {info && (info.airs.length > 0 || info.activity.length > 0) && (
                    <span className="flex gap-1" aria-hidden="true">
                      {info.airs.length > 0 && <span className="size-1.5 rounded-full bg-accent" />}
                      {info.activity.length > 0 && (
                        <span className="size-1.5 rounded-full bg-text-tertiary" />
                      )}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>

        <aside className="rounded-md border border-border-subtle bg-bg-surface p-4">
          <h2 className="text-sm font-semibold text-text-primary">{t("calendar.events")}</h2>
          <p className="mt-1 text-xs text-text-tertiary">{selectedKey}</p>
          {dayInfo && (dayInfo.airs.length > 0 || dayInfo.activity.length > 0) ? (
            <ul className="mt-3 flex flex-col gap-3">
              {dayInfo.airs.map((item, index) => (
                <EventRow
                  key={`air-${index}`}
                  item={item}
                  kindLabel={item.label ?? ""}
                  time={null}
                  variant="air"
                />
              ))}
              {dayInfo.activity.map((item, index) => (
                <EventRow
                  key={`activity-${index}`}
                  item={item}
                  kindLabel={t(`calendar.kind_${item.kind}`)}
                  time={item.time}
                  variant="activity"
                />
              ))}
            </ul>
          ) : (
            <p className="mt-2 text-sm text-text-tertiary">{t("calendar.noEvents")}</p>
          )}
        </aside>
      </div>
    </section>
  );
}
