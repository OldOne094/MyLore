import { BarChart3, RefreshCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { useStatsSummaryQuery, type StatCount, type StatsView } from "./api";

/* MISSION-080 — Stats page (REQ-STAT-001). A calm overview of the tracked
   library: stat cards with tabular numbers (titles, completion, average
   rating, favorites, consumption) plus four small distribution charts. Charts
   are hand-rolled horizontal bars so the page stays dependency-light, RTL-safe
   (logical properties), and uses the shared accent token. */

const EMPTY_STATS: StatsView = {
  total: 0,
  status_counts: [],
  content_type_counts: [],
  rating_counts: [],
  avg_rating: null,
  favorites: 0,
  completed_media: 0,
  completion_rate: null,
  avg_percent: null,
  consumed_minutes: 0,
  consumed_hours: 0,
  consumed_pages: 0,
  year_counts: [],
};

function formatHours(hours: number): string {
  const rounded = Math.round(hours * 10) / 10;
  return Number.isInteger(rounded) ? String(Math.round(rounded)) : rounded.toFixed(1);
}

function StatsSkeleton() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("nav.stats")} role="status" className="px-5 py-5">
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {Array.from({ length: 7 }, (_, index) => (
          <div key={index} className="rounded-md bg-bg-surface p-4">
            <Skeleton className="h-3 w-20" />
            <Skeleton className="mt-3 h-6 w-12" />
          </div>
        ))}
      </div>
      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        {Array.from({ length: 4 }, (_, index) => (
          <div key={index} className="rounded-md bg-bg-surface p-4">
            <Skeleton className="h-4 w-24" />
            <div className="mt-3 flex flex-col gap-2">
              <Skeleton className="h-2.5" />
              <Skeleton className="h-2.5 w-4/5" />
              <Skeleton className="h-2.5 w-3/5" />
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function StatCard({ label, value, suffix }: { label: string; value: string; suffix?: string }) {
  return (
    <div className="rounded-md border border-border-subtle bg-bg-surface p-4">
      <p className="text-xs font-medium text-text-secondary">{label}</p>
      <p className="mt-1.5 truncate text-2xl font-semibold leading-none text-text-primary tabular-nums">
        {value}
        {suffix ? (
          <span className="ms-1 text-sm font-normal text-text-tertiary">{suffix}</span>
        ) : null}
      </p>
    </div>
  );
}

function DistributionChart({
  title,
  rows,
  format,
}: {
  title: string;
  rows: StatCount[];
  format: (key: string) => string;
}) {
  const { t } = useTranslation();
  const visible = rows.filter((row) => row.count > 0);
  const max = Math.max(1, ...visible.map((row) => row.count));
  return (
    <section className="rounded-md border border-border-subtle bg-bg-surface p-4">
      <h2 className="text-sm font-semibold text-text-primary">{title}</h2>
      {visible.length === 0 ? (
        <p className="mt-2 text-sm text-text-tertiary">{t("stats.noData")}</p>
      ) : (
        <ul className="mt-3 flex flex-col gap-2">
          {visible.map((row) => (
            <li key={row.key} className="flex items-center gap-2">
              <span className="w-24 shrink-0 truncate text-xs text-text-secondary">
                {format(row.key)}
              </span>
              <div
                className="h-2 flex-1 overflow-hidden rounded-full bg-accent/20"
                aria-hidden="true"
              >
                <div
                  className="h-full rounded-full bg-accent transition-[width] duration-150 ease-out"
                  style={{ width: `${(row.count / max) * 100}%` }}
                />
              </div>
              <span className="w-8 shrink-0 text-end text-xs tabular-nums text-text-tertiary">
                {row.count}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function StatsPage() {
  const { t } = useTranslation();
  const { data, isLoading, isError, refetch } = useStatsSummaryQuery();

  if (isLoading) return <StatsSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={BarChart3}
        title={t("stats.errorTitle")}
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

  const stats = data ?? EMPTY_STATS;

  if (stats.total === 0) {
    return (
      <EmptyState icon={BarChart3} title={t("stats.emptyTitle")} hint={t("stats.emptyHint")} />
    );
  }

  const completionRate =
    stats.completion_rate === null ? "—" : `${Math.round(stats.completion_rate * 100)}%`;
  const avgRating = stats.avg_rating === null ? "—" : stats.avg_rating.toFixed(1);

  return (
    <section aria-label={t("nav.stats")} className="px-5 py-5">
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard label={t("stats.total")} value={String(stats.total)} />
        <StatCard label={t("stats.completed")} value={String(stats.completed_media)} />
        <StatCard label={t("stats.completionRate")} value={completionRate} />
        <StatCard label={t("stats.avgRating")} value={avgRating} suffix={t("stats.ratingSuffix")} />
        <StatCard label={t("stats.favorites")} value={String(stats.favorites)} />
        <StatCard
          label={t("stats.watched")}
          value={formatHours(stats.consumed_hours)}
          suffix={t("stats.hoursSuffix")}
        />
        <StatCard
          label={t("stats.read")}
          value={String(stats.consumed_pages)}
          suffix={t("stats.pagesSuffix")}
        />
      </div>
      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <DistributionChart
          title={t("stats.byStatus")}
          rows={stats.status_counts}
          format={(key) => t(`coreStatus.${key}`, { defaultValue: key })}
        />
        <DistributionChart
          title={t("stats.byType")}
          rows={stats.content_type_counts}
          format={(key) => t(`contentType.${key}`, { defaultValue: key })}
        />
        <DistributionChart
          title={t("stats.byRating")}
          rows={stats.rating_counts}
          format={(key) => key}
        />
        <DistributionChart
          title={t("stats.byYear")}
          rows={stats.year_counts}
          format={(key) => key}
        />
      </div>
    </section>
  );
}
