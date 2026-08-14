import { Activity, Hand, RefreshCcw, Zap } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, Skeleton, useToast } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useSetAutoTrack, useSetStatus, useTrackingQuery } from "./tracking";

/* MISSION-048/052 — Tracking tab. Exclusive status picker (7 core statuses,
   colored per the status palette) plus started/finished dates, the repeat run
   counter, the Normal (autoTrack) vs Manual mode toggle, and the DNF progress
   for dropped titles. The status engine on the backend owns transitions;
   marking all nodes in the content tree auto-completes in Normal mode,
   surfaced here via cache invalidation from `useNodeProgress`. */

interface DnfProgress {
  percent: number | null;
  next_label: string | null;
}

function dnfValue(progress: DnfProgress): string {
  const parts = [
    progress.next_label,
    progress.percent != null ? `${progress.percent}%` : null,
  ].filter(Boolean) as string[];
  return parts.join(" · ");
}

const STATUS_ORDER = [
  "planned",
  "in_progress",
  "completed",
  "on_hold",
  "dropped",
  "repeat",
  "wishlist",
] as const;

const STATUS_PILLS: Record<string, { selected: string; hover: string }> = {
  planned: {
    selected: "border-status-planned/60 bg-status-planned/12 text-status-planned",
    hover: "hover:border-status-planned/40 hover:text-status-planned",
  },
  in_progress: {
    selected: "border-status-inprogress/60 bg-status-inprogress/12 text-status-inprogress",
    hover: "hover:border-status-inprogress/40 hover:text-status-inprogress",
  },
  completed: {
    selected: "border-status-completed/60 bg-status-completed/12 text-status-completed",
    hover: "hover:border-status-completed/40 hover:text-status-completed",
  },
  on_hold: {
    selected: "border-status-onhold/60 bg-status-onhold/12 text-status-onhold",
    hover: "hover:border-status-onhold/40 hover:text-status-onhold",
  },
  dropped: {
    selected: "border-status-dropped/60 bg-status-dropped/12 text-status-dropped",
    hover: "hover:border-status-dropped/40 hover:text-status-dropped",
  },
  repeat: {
    selected: "border-status-repeat/60 bg-status-repeat/12 text-status-repeat",
    hover: "hover:border-status-repeat/40 hover:text-status-repeat",
  },
  wishlist: {
    selected: "border-status-planned/60 bg-status-planned/12 text-status-planned",
    hover: "hover:border-status-planned/40 hover:text-status-planned",
  },
};

function formatDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

function MetaItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-xs uppercase tracking-wide text-text-tertiary">{label}</span>
      <span className="text-sm text-text-primary">{value}</span>
    </div>
  );
}

function TrackingSkeleton() {
  return (
    <div role="status" aria-label="tracking" className="flex flex-col gap-6">
      <div className="flex flex-col gap-2">
        <Skeleton className="h-4 w-40" />
        <Skeleton className="h-3 w-72" />
        <div className="mt-2 flex flex-wrap gap-2">
          {[0, 1, 2, 3, 4].map((i) => (
            <Skeleton key={i} className="h-8 w-24 rounded-full" />
          ))}
        </div>
      </div>
      <div className="flex flex-col gap-2">
        <Skeleton className="h-4 w-24" />
        <Skeleton className="h-4 w-48" />
      </div>
    </div>
  );
}

export function TrackingTab({ mediaId }: { mediaId: string }) {
  const { t } = useTranslation();
  const { data, isPending, isError, refetch } = useTrackingQuery(mediaId);
  const setStatus = useSetStatus();
  const setAutoTrack = useSetAutoTrack();
  const toast = useToast();

  const current = data?.core_status ?? null;
  const autoTrack = data?.auto_track ?? true;

  const apply = (coreStatus: string) => {
    setStatus.mutate(
      { media_id: mediaId, core_status: coreStatus },
      { onError: () => toast.error({ title: t("tracking.setErrorToast") }) },
    );
  };

  const applyMode = (enabled: boolean) => {
    setAutoTrack.mutate(
      { media_id: mediaId, auto_track: enabled },
      { onError: () => toast.error({ title: t("tracking.setModeErrorToast") }) },
    );
  };

  if (isPending) return <TrackingSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={RefreshCcw}
        title={t("tracking.loadErrorTitle")}
        hint={t("tracking.loadErrorHint")}
        action={<Button onClick={() => refetch()}>{t("tracking.retry")}</Button>}
      />
    );
  }

  return (
    <div className="flex max-w-xl flex-col gap-8">
      <section aria-labelledby="tracking-status-heading">
        <h2 id="tracking-status-heading" className="text-sm font-medium text-text-primary">
          {t("tracking.statusTitle")}
        </h2>
        <p className="mt-1 text-xs text-text-secondary">{t("tracking.statusHint")}</p>
        <div
          role="group"
          aria-label={t("tracking.statusTitle")}
          className="mt-4 flex flex-wrap gap-2"
        >
          {STATUS_ORDER.map((status) => {
            const pill = STATUS_PILLS[status];
            const selected = current === status;
            return (
              <button
                key={status}
                type="button"
                aria-pressed={selected}
                disabled={setStatus.isPending}
                onClick={() => apply(status)}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-sm transition-colors duration-150 ease-out disabled:opacity-60",
                  selected
                    ? cn(pill.selected, "ring-1 ring-inset ring-current")
                    : cn("border-border-subtle text-text-secondary", pill.hover),
                )}
              >
                {selected ? <Activity size={13} aria-hidden="true" /> : null}
                {t(`coreStatus.${status}`)}
              </button>
            );
          })}
        </div>
      </section>

      <section aria-labelledby="tracking-mode-heading">
        <h2 id="tracking-mode-heading" className="text-sm font-medium text-text-primary">
          {t("tracking.modeTitle")}
        </h2>
        <p className="mt-1 text-xs text-text-secondary">{t("tracking.modeHint")}</p>
        <div
          role="group"
          aria-label={t("tracking.modeTitle")}
          className="mt-4 inline-flex rounded-lg border border-border-subtle bg-bg-surface p-0.5"
        >
          {(
            [
              { enabled: true, label: t("tracking.modeNormal"), icon: Zap },
              { enabled: false, label: t("tracking.modeManual"), icon: Hand },
            ] as const
          ).map(({ enabled, label, icon: Icon }) => {
            const selected = autoTrack === enabled;
            return (
              <button
                key={label}
                type="button"
                aria-pressed={selected}
                disabled={setAutoTrack.isPending}
                onClick={() => applyMode(enabled)}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm transition-colors duration-150 ease-out disabled:opacity-60",
                  selected
                    ? "bg-bg-raised text-text-primary shadow-sm"
                    : "text-text-tertiary hover:text-text-secondary",
                )}
              >
                <Icon size={14} aria-hidden="true" />
                {label}
              </button>
            );
          })}
        </div>
      </section>

      <section aria-label={t("tracking.metaTitle")}>
        {data ? (
          <dl className="grid grid-cols-1 gap-x-8 gap-y-5 sm:grid-cols-3">
            {data.started_at ? (
              <MetaItem label={t("tracking.startedAt")} value={formatDate(data.started_at)} />
            ) : null}
            {data.finished_at ? (
              <MetaItem label={t("tracking.finishedAt")} value={formatDate(data.finished_at)} />
            ) : null}
            {data.core_status === "repeat" ? (
              <MetaItem label={t("tracking.repeatCount")} value={`#${data.repeat_count}`} />
            ) : null}
            {data.core_status === "dropped" &&
            data.progress &&
            (data.progress.next_label || data.progress.percent != null) ? (
              <MetaItem label={t("tracking.dnfTitle")} value={dnfValue(data.progress)} />
            ) : null}
          </dl>
        ) : (
          <p className="text-sm text-text-tertiary">{t("tracking.untrackedHint")}</p>
        )}
      </section>
    </div>
  );
}
