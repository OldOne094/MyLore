import { useState } from "react";
import { Library, Plus, RefreshCcw, SlidersHorizontal } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { useMediaFacetsQuery, useMediaListQuery } from "./api";
import { AddMediaDialog } from "./AddMediaDialog";
import { LibraryViewSwitcher, type LibraryView } from "./LibraryViewSwitcher";
import { VirtualizedLibrary } from "./VirtualizedLibrary";
import { LibraryFilterBar } from "./LibraryFilterBar";
import {
  activeFilterCount,
  DEFAULT_FILTERS,
  DEFAULT_SORT,
  filtersToArgs,
  type LibraryFilters,
  type LibrarySort,
} from "./filters";
import type { LibraryGroupBy } from "./grouping";

/* MISSION-040/041 — Library landing view: the add flow when empty, a toolbar
   with the Grid / List / Compact density switcher when populated, filter
   panel (type/format/status/genre/tag/year/favorite) + sort menu + group-by,
   per-view skeletons while loading, and a retry on failure. Rendering is
   virtualized. */

function AddTitleTrigger() {
  const { t } = useTranslation();
  return (
    <AddMediaDialog
      trigger={
        <Button>
          <Plus size={16} aria-hidden="true" />
          {t("library.add")}
        </Button>
      }
    />
  );
}

function LibrarySkeleton({ view }: { view: LibraryView }) {
  if (view !== "grid") {
    const dense = view === "compact";
    return (
      <div role="status" aria-label="Loading library" className="px-6 pt-6">
        {Array.from({ length: 8 }, (_, index) => (
          <div
            key={index}
            className={`mb-2 flex items-center gap-3 rounded-md bg-bg-surface ${dense ? "px-2 py-1" : "px-3 py-2"}`}
          >
            <Skeleton className={dense ? "size-7" : "size-10"} />
            <Skeleton className="h-4 flex-1" />
            <Skeleton className="h-3 w-16" />
          </div>
        ))}
      </div>
    );
  }

  return (
    <div
      role="status"
      aria-label="Loading library"
      className="grid grid-cols-2 gap-4 p-6 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6"
    >
      {Array.from({ length: 12 }, (_, index) => (
        <div key={index} className="flex flex-col gap-2">
          <Skeleton className="aspect-[2/3] w-full" />
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-3 w-1/2" />
        </div>
      ))}
    </div>
  );
}

function EmptyLibrary() {
  const { t } = useTranslation();
  return (
    <EmptyState
      icon={Library}
      title={t("library.emptyTitle")}
      hint={t("library.emptyHint")}
      action={<AddTitleTrigger />}
    />
  );
}

export function LibraryPage() {
  const { t } = useTranslation();
  const [view, setView] = useState<LibraryView>("grid");
  const [filters, setFilters] = useState<LibraryFilters>(DEFAULT_FILTERS);
  const [sort, setSort] = useState<LibrarySort>(DEFAULT_SORT);
  const [groupBy, setGroupBy] = useState<LibraryGroupBy>("none");

  const queryArgs = filtersToArgs(filters, sort);
  const { data, isLoading, isError, refetch } = useMediaListQuery(queryArgs);
  const { data: facets } = useMediaFacetsQuery();

  if (isLoading) return <LibrarySkeleton view={view} />;

  if (isError) {
    return (
      <EmptyState
        icon={Library}
        title={t("library.errorTitle")}
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

  const items = data ?? [];
  const hasActiveFilters = activeFilterCount(filters) > 0;
  if (items.length === 0 && !hasActiveFilters) return <EmptyLibrary />;

  return (
    <section aria-label={t("nav.library")} className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-border-subtle px-5 py-3">
        <span className="text-sm tabular-nums text-text-secondary">
          {t("shell.status.counts", { count: items.length })}
        </span>
        <div className="flex shrink-0 items-center gap-2">
          <AddTitleTrigger />
          <LibraryViewSwitcher view={view} onChange={setView} />
        </div>
      </div>
      <LibraryFilterBar
        filters={filters}
        sort={sort}
        groupBy={groupBy}
        facets={facets}
        onFiltersChange={setFilters}
        onSortChange={setSort}
        onGroupByChange={setGroupBy}
      />
      {items.length === 0 ? (
        <EmptyState
          icon={SlidersHorizontal}
          title={t("library.noResultsTitle")}
          hint={t("library.noResultsHint")}
          action={
            <Button variant="secondary" onClick={() => setFilters(DEFAULT_FILTERS)}>
              {t("library.filtersClear")}
            </Button>
          }
        />
      ) : (
        <VirtualizedLibrary view={view} items={items} groupBy={groupBy} />
      )}
    </section>
  );
}
