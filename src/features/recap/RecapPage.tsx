import { Flame, RefreshCcw, Sparkles, Trophy } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useRecapYearQuery, type RecapMedia, type YearRecap } from "./api";

/* MISSION-082 — Year-in-review recap (REQ-STAT-001 extension). A celebratory
   whole-year summary: headline totals from the activity trail, a monthly
   "finishes" bar chart (hand-rolled, RTL-safe via logical properties + dir),
   top genres of finished media and the most-active titles. Year is switchable
   via a select; everything reuses the tabular-numbers + accent tokens. */

const MIN_YEAR = 2000;

function monthName(language: string, index: number, style: "short" | "long"): string {
  return new Intl.DateTimeFormat(language, { month: style }).format(new Date(2000, index, 1));
}

function RecapSkeleton() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("nav.recap")} role="status" className="px-5 py-5">
      <Skeleton className="h-6 w-48" />
      <div className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
        {Array.from({ length: 5 }, (_, index) => (
          <div key={index} className="rounded-md border border-border-subtle bg-bg-surface p-4">
            <Skeleton className="h-3 w-20" />
            <Skeleton className="mt-3 h-6 w-12" />
          </div>
        ))}
      </div>
      <div className="mt-4 grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
        <div className="rounded-md border border-border-subtle bg-bg-surface p-4">
          <Skeleton className="h-4 w-32" />
          <Skeleton className="mt-4 h-40" />
        </div>
        <div className="rounded-md border border-border-subtle bg-bg-surface p-4">
          <Skeleton className="h-4 w-24" />
          <div className="mt-3 flex flex-col gap-2">
            <Skeleton className="h-2.5" />
            <Skeleton className="h-2.5 w-4/5" />
            <Skeleton className="h-2.5 w-3/5" />
          </div>
        </div>
      </div>
    </section>
  );
}

function StatCard({
  icon: Icon,
  label,
  value,
  suffix,
}: {
  icon?: typeof Flame;
  label: string;
  value: string;
  suffix?: string;
}) {
  return (
    <div className="rounded-md border border-border-subtle bg-bg-surface p-4">
      <p className="flex items-center gap-1.5 text-xs font-medium text-text-secondary">
        {Icon && <Icon size={14} aria-hidden="true" className="text-text-tertiary" />}
        {label}
      </p>
      <p className="mt-1.5 truncate text-2xl font-semibold leading-none text-text-primary tabular-nums">
        {value}
        {suffix ? (
          <span className="ms-1 text-sm font-normal text-text-tertiary">{suffix}</span>
        ) : null}
      </p>
    </div>
  );
}

function MonthChart({ recap }: { recap: YearRecap }) {
  const { t, i18n } = useTranslation();
  const max = Math.max(1, ...recap.by_month);
  return (
    <section
      aria-label={t("recap.chart")}
      className="rounded-md border border-border-subtle bg-bg-surface p-4"
    >
      <h2 className="text-sm font-semibold text-text-primary">{t("recap.chart")}</h2>
      {max === 1 && recap.by_month.every((c) => c === 0) ? (
        <p className="mt-2 text-sm text-text-tertiary">{t("recap.noCompletions")}</p>
      ) : (
        <div className="mt-4 flex h-40 items-end gap-1">
          {recap.by_month.map((count, index) => {
            const isBest = recap.best_month === index + 1;
            return (
              <div
                key={index}
                className="flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-1"
              >
                {count > 0 && (
                  <span className="text-[10px] tabular-nums text-text-secondary">{count}</span>
                )}
                <div
                  className={cn(
                    "w-full max-w-7 rounded-t-sm transition-[height] duration-150 ease-out",
                    isBest ? "bg-accent" : "bg-accent/40",
                  )}
                  style={{ height: `${(count / max) * 100}%` }}
                  title={monthName(i18n.language, index, "long")}
                />
                <span className="text-[10px] text-text-tertiary">
                  {monthName(i18n.language, index, "short")}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

function TopGenres({ genres }: { genres: YearRecap["top_genres"] }) {
  const { t } = useTranslation();
  return (
    <section
      aria-label={t("recap.topGenres")}
      className="rounded-md border border-border-subtle bg-bg-surface p-4"
    >
      <h2 className="text-sm font-semibold text-text-primary">{t("recap.topGenres")}</h2>
      {genres.length === 0 ? (
        <p className="mt-2 text-sm text-text-tertiary">{t("recap.noCompletions")}</p>
      ) : (
        <ul className="mt-3 flex flex-wrap gap-1.5">
          {genres.map((genre) => (
            <li
              key={genre.name}
              className="inline-flex items-center gap-1 rounded-full border border-border-strong bg-bg-raised px-2.5 py-1 text-xs text-text-secondary"
            >
              {genre.name}
              <span className="tabular-nums text-text-tertiary">{genre.count}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function MostActive({ items }: { items: RecapMedia[] }) {
  const { t } = useTranslation();
  return (
    <section
      aria-label={t("recap.mostActive")}
      className="rounded-md border border-border-subtle bg-bg-surface p-4"
    >
      <h2 className="text-sm font-semibold text-text-primary">{t("recap.mostActive")}</h2>
      {items.length === 0 ? (
        <p className="mt-2 text-sm text-text-tertiary">{t("recap.noCompletions")}</p>
      ) : (
        <ol className="mt-3 flex flex-col gap-2.5">
          {items.map((item, index) => (
            <li key={index} className="flex items-center gap-2">
              <span className="w-5 shrink-0 text-end text-xs tabular-nums text-text-tertiary">
                {index + 1}
              </span>
              <span className="min-w-0 flex-1 truncate text-sm text-text-primary">
                {item.media_id ? (
                  <Link
                    className="transition-colors duration-150 ease-out hover:text-accent"
                    to={`/library/${item.media_id}`}
                  >
                    {item.title}
                  </Link>
                ) : (
                  item.title
                )}
              </span>
              {item.content_type && (
                <span className="shrink-0 text-xs text-text-tertiary">
                  {t(`contentType.${item.content_type}`, { defaultValue: item.content_type })}
                </span>
              )}
              <span className="shrink-0 text-xs tabular-nums text-text-tertiary">
                ×{item.activity_count}
              </span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

export function RecapPage() {
  const { t, i18n } = useTranslation();
  const [year, setYear] = useState(() => new Date().getFullYear());
  const { data, isLoading, isError, refetch } = useRecapYearQuery(year);
  const yearReady = data?.year === year;

  const years: number[] = [];
  for (let y = new Date().getFullYear(); y >= MIN_YEAR; y -= 1) years.push(y);

  if (isError) {
    return (
      <EmptyState
        icon={Trophy}
        title={t("recap.errorTitle")}
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

  if (isLoading || !yearReady) return <RecapSkeleton />;

  const isQuiet =
    data.totals.added +
      data.totals.started +
      data.totals.completed +
      data.totals.reviewed +
      data.totals.progress ===
    0;

  if (isQuiet) {
    return (
      <EmptyState
        icon={Trophy}
        title={t("recap.emptyTitle")}
        hint={t("recap.emptyHint", { year: String(year) })}
      />
    );
  }

  const bestMonth =
    data.best_month === null ? "—" : monthName(i18n.language, data.best_month - 1, "long");

  return (
    <section aria-label={t("nav.recap")} className="px-5 py-5">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <h2 className="text-lg font-semibold text-text-primary">
          {t("recap.title", { year: String(year) })}
        </h2>
        <label className="flex items-center gap-2 text-sm text-text-secondary">
          {t("recap.pickYear")}
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

      <div className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
        <StatCard label={t("recap.added")} value={String(data.totals.added)} />
        <StatCard label={t("recap.started")} value={String(data.totals.started)} />
        <StatCard label={t("recap.completed")} value={String(data.totals.completed)} />
        <StatCard label={t("recap.reviewed")} value={String(data.totals.reviewed)} />
        <StatCard label={t("recap.progress")} value={String(data.totals.progress)} />
      </div>

      <div className="mt-4 grid gap-4 sm:grid-cols-2">
        <StatCard icon={Sparkles} label={t("recap.bestMonth")} value={bestMonth} />
        <StatCard
          icon={Flame}
          label={t("recap.longestStreak")}
          value={String(data.longest_streak)}
          suffix={t("recap.days")}
        />
      </div>

      <div className="mt-4 grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
        <MonthChart recap={data} />
        <aside className="flex flex-col gap-4">
          <TopGenres genres={data.top_genres} />
          <MostActive items={data.top_media} />
        </aside>
      </div>
    </section>
  );
}
