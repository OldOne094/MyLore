import { LayoutDashboard, Plus, RefreshCcw, Search, Zap } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { AddMediaDialog } from "@/features/library/AddMediaDialog";
import { MediaRow } from "@/features/library/MediaRow";
import { useDashboardSummaryQuery, type MediaListItem } from "./api";

/* MISSION-050 — Dashboard home (REQ-DASH-001). A calm, empty-by-default widget
   grid: Continue reading/watching (resumes the in-progress titles via the
   shared quick controls), Recently completed, Recently added, and Quick
   actions. Widgets are not draggable in the MVP and stay simple text+list
   cards; each list reuses the library MediaRow so every row carries its
   progress bar and next-unit control. */

function DashboardSkeleton() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("nav.dashboard")} role="status" className="px-5 py-5">
      <div className="grid gap-4 lg:grid-cols-2">
        <div className="flex flex-wrap gap-2">
          <Skeleton className="h-9 w-28" />
          <Skeleton className="h-9 w-28" />
          <Skeleton className="h-9 w-28" />
        </div>
        <div className="rounded-md bg-bg-surface p-3 lg:col-span-2">
          <Skeleton className="h-4 w-40" />
          <div className="mt-3 flex flex-col gap-1">
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
          </div>
        </div>
        <div className="rounded-md bg-bg-surface p-3">
          <Skeleton className="h-4 w-40" />
          <div className="mt-3 flex flex-col gap-1">
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
          </div>
        </div>
        <div className="rounded-md bg-bg-surface p-3">
          <Skeleton className="h-4 w-40" />
          <div className="mt-3 flex flex-col gap-1">
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
          </div>
        </div>
      </div>
    </section>
  );
}

function Widget({ title, empty, items }: { title: string; empty: string; items: MediaListItem[] }) {
  return (
    <section className="rounded-md border border-border-subtle bg-bg-surface p-4">
      <h2 className="text-sm font-semibold text-text-primary">{title}</h2>
      {items.length === 0 ? (
        <p className="mt-2 text-sm text-text-tertiary">{empty}</p>
      ) : (
        <ul className="mt-3 flex flex-col gap-1">
          {items.map((item) => (
            <li key={item.id}>
              <MediaRow item={item} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function DashboardPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data, isLoading, isError, refetch } = useDashboardSummaryQuery();

  const openQuickCapture = () => window.dispatchEvent(new Event("mylore:open-quick-capture"));

  if (isLoading) return <DashboardSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={LayoutDashboard}
        title={t("dashboard.errorTitle")}
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

  const summary = data ?? { continue_watching: [], recently_completed: [], recently_added: [] };

  return (
    <section aria-label={t("nav.dashboard")} className="px-5 py-5">
      <div className="grid gap-4 lg:grid-cols-2">
        <section className="rounded-md border border-border-subtle bg-bg-surface p-4 lg:col-span-2">
          <h2 className="text-sm font-semibold text-text-primary">{t("dashboard.quickActions")}</h2>
          <div className="mt-3 flex flex-wrap gap-2">
            <AddMediaDialog
              trigger={
                <Button>
                  <Plus size={16} aria-hidden="true" />
                  {t("dashboard.quickAdd")}
                </Button>
              }
            />
            <Button variant="secondary" onClick={openQuickCapture}>
              <Zap size={16} aria-hidden="true" />
              {t("dashboard.quickCapture")}
            </Button>
            <Button variant="secondary" onClick={() => navigate("/search")}>
              <Search size={16} aria-hidden="true" />
              {t("dashboard.searchLibrary")}
            </Button>
          </div>
        </section>

        <Widget
          title={t("dashboard.continue")}
          empty={t("dashboard.continueEmpty")}
          items={summary.continue_watching}
        />
        <Widget
          title={t("dashboard.completed")}
          empty={t("dashboard.completedEmpty")}
          items={summary.recently_completed}
        />
        <Widget
          title={t("dashboard.added")}
          empty={t("dashboard.addedEmpty")}
          items={summary.recently_added}
        />
      </div>
    </section>
  );
}
