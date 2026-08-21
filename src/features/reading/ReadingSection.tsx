import { BookOpen, RefreshCcw } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Skeleton } from "@/components/ui";
import { DistributionChart } from "@/features/stats/DistributionChart";
import { useReadingRecapQuery, type ReadingRecap } from "./api";

/* MISSION-083 — Reading recap (REQ-STAT-001 addition): a StoryGraph-style
   section under the Stats page. Pages/chapters consumed per month of a chosen
   year (book pages weighed by page count, bucketed by local time), the year
   totals including distinct finished reading media, and all-time taste
   distributions — mood set, pace and format — from review metadata and the
   tracked library. Charts are hand-rolled (month bars + the shared horizontal
   DistributionChart) and stay RTL-safe via logical properties. */

const MIN_YEAR = 2000;

function monthName(language: string, index: number, style: "short" | "long"): string {
  return new Intl.DateTimeFormat(language, { month: style }).format(new Date(2000, index, 1));
}

function ReadingSkeleton() {
  return (
    <div role="status" className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {Array.from({ length: 3 }, (_, index) => (
        <div key={index} className="rounded-md border border-border-subtle bg-bg-surface p-4">
          <Skeleton className="h-3 w-20" />
          <Skeleton className="mt-3 h-6 w-12" />
        </div>
      ))}
      <div className="rounded-md border border-border-subtle bg-bg-surface p-4 sm:col-span-2 lg:col-span-3">
        <Skeleton className="h-4 w-32" />
        <Skeleton className="mt-4 h-40" />
      </div>
    </div>
  );
}

function ReadingCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border-subtle bg-bg-surface p-4">
      <p className="text-xs font-medium text-text-secondary">{label}</p>
      <p className="mt-1.5 truncate text-2xl font-semibold leading-none text-text-primary tabular-nums">
        {value}
      </p>
    </div>
  );
}

function MonthBars({ title, values }: { title: string; values: number[] }) {
  const { t, i18n } = useTranslation();
  const max = Math.max(1, ...values);
  return (
    <section
      aria-label={title}
      className="rounded-md border border-border-subtle bg-bg-surface p-4"
    >
      <h2 className="text-sm font-semibold text-text-primary">{title}</h2>
      {values.every((value) => value === 0) ? (
        <p className="mt-2 text-sm text-text-tertiary">{t("reading.noData")}</p>
      ) : (
        <div className="mt-4 flex h-40 items-end gap-1">
          {values.map((value, index) => (
            <div
              key={index}
              className="flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-1"
            >
              {value > 0 && (
                <span className="text-[10px] tabular-nums text-text-secondary">{value}</span>
              )}
              <div
                className="w-full max-w-7 rounded-t-sm bg-accent/40 transition-[height] duration-150 ease-out"
                style={{ height: `${(value / max) * 100}%` }}
                title={monthName(i18n.language, index, "long")}
              />
              <span className="text-[10px] text-text-tertiary">
                {monthName(i18n.language, index, "short")}
              </span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function ReadingRecapView({ recap }: { recap: ReadingRecap }) {
  const { t } = useTranslation();
  const isQuiet =
    recap.totals.pages + recap.totals.chapters + recap.totals.finished === 0 &&
    recap.mood_counts.length === 0 &&
    recap.pace_counts.length === 0 &&
    recap.format_counts.length === 0;

  if (isQuiet) {
    return (
      <p className="mt-4 rounded-md border border-border-subtle bg-bg-surface p-4 text-sm text-text-tertiary">
        {t("reading.noData")}
      </p>
    );
  }

  return (
    <>
      <div className="mt-4 grid gap-4 sm:grid-cols-3">
        <ReadingCard label={t("reading.pages")} value={String(recap.totals.pages)} />
        <ReadingCard label={t("reading.chapters")} value={String(recap.totals.chapters)} />
        <ReadingCard label={t("reading.finished")} value={String(recap.totals.finished)} />
      </div>
      <div className="mt-4 grid gap-4 sm:grid-cols-2">
        <MonthBars
          title={t("reading.pagesChart")}
          values={recap.by_month.map((month) => month.pages)}
        />
        <MonthBars
          title={t("reading.chaptersChart")}
          values={recap.by_month.map((month) => month.chapters)}
        />
      </div>
      <div className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <DistributionChart
          title={t("reading.moods")}
          rows={recap.mood_counts}
          emptyLabel={t("reading.noData")}
          format={(key) => t(`mood.${key}`, { defaultValue: key })}
        />
        <DistributionChart
          title={t("reading.pace")}
          rows={recap.pace_counts}
          emptyLabel={t("reading.noData")}
          format={(key) => t(`pace.${key}`, { defaultValue: key })}
        />
        <DistributionChart
          title={t("reading.formats")}
          rows={recap.format_counts}
          emptyLabel={t("reading.noData")}
          format={(key) => key}
        />
      </div>
    </>
  );
}

export function ReadingSection() {
  const { t } = useTranslation();
  const [year, setYear] = useState(() => new Date().getFullYear());
  const { data, isLoading, isError, refetch } = useReadingRecapQuery(year);
  const yearReady = data?.year === year;

  const years: number[] = [];
  for (let y = new Date().getFullYear(); y >= MIN_YEAR; y -= 1) years.push(y);

  return (
    <section aria-label={t("reading.title")} className="mt-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <h2 className="flex items-center gap-2 text-lg font-semibold text-text-primary">
          <BookOpen size={18} aria-hidden="true" className="text-text-tertiary" />
          {t("reading.title")}
        </h2>
        <label className="flex items-center gap-2 text-sm text-text-secondary">
          {t("reading.pickYear")}
          <select
            value={year}
            onChange={(event) => setYear(Number(event.target.value))}
            className="rounded-md border border-border-strong bg-bg-surface px-2 py-1 text-sm text-text-primary focus:border-accent focus:outline-none"
          >
            {years.map((y) => (
              <option key={y} value={y}>
                {y}
              </option>
            ))}
          </select>
        </label>
      </div>

      {isError ? (
        <div className="mt-4 flex flex-wrap items-center justify-between gap-3 rounded-md border border-border-subtle bg-bg-surface p-4">
          <p className="text-sm text-text-secondary">{t("reading.errorTitle")}</p>
          <Button variant="secondary" onClick={() => void refetch()}>
            <RefreshCcw size={16} aria-hidden="true" />
            {t("library.retry")}
          </Button>
        </div>
      ) : isLoading || !yearReady ? (
        <ReadingSkeleton />
      ) : (
        <ReadingRecapView recap={data} />
      )}
    </section>
  );
}
