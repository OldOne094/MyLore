import { useState } from "react";
import { CheckSquare, Library, Plus, RefreshCcw, SlidersHorizontal, Upload, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { useMediaFacetsQuery, useMediaListQuery } from "./api";
import { AddMediaDialog } from "./AddMediaDialog";
import { ImportFileDialog } from "@/features/import/ImportFileDialog";
import { LibraryViewSwitcher, type LibraryView } from "./LibraryViewSwitcher";
import { VirtualizedLibrary } from "./VirtualizedLibrary";
import { LibraryFilterBar } from "./LibraryFilterBar";
import { BulkActionBar } from "./BulkActionBar";
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
   virtualized. MISSION-045 adds bulk-select mode: a Select toggle flips cards
   and rows into checkboxes and a bottom action bar offers status / tag / list
   / delete (export arrives later). MISSION-078 lets those actions apply to the
   whole filtered selection (server-resolved) with a change summary. */

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

function ImportFileTrigger() {
  const { t } = useTranslation();
  return (
    <ImportFileDialog
      trigger={
        <Button variant="secondary">
          <Upload size={16} aria-hidden="true" />
          {t("library.import")}
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
      action={
        <div className="flex items-center justify-center gap-2">
          <AddTitleTrigger />
          <ImportFileTrigger />
        </div>
      }
    />
  );
}

export function LibraryPage() {
  const { t } = useTranslation();
  const [view, setView] = useState<LibraryView>("grid");
  const [filters, setFilters] = useState<LibraryFilters>(DEFAULT_FILTERS);
  const [sort, setSort] = useState<LibrarySort>(DEFAULT_SORT);
  const [groupBy, setGroupBy] = useState<LibraryGroupBy>("none");
  const [selectMode, setSelectMode] = useState(false);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());

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

  const exitSelect = () => {
    setSelectMode(false);
    setSelected(new Set());
  };

  const toggleItem = (id: string) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleAll = () => {
    setSelected((previous) => {
      const allSelected = previous.size === items.length;
      if (allSelected) return new Set();
      return new Set(items.map((item) => item.id));
    });
  };

  const allSelected = items.length > 0 && selected.size === items.length;

  return (
    <section aria-label={t("nav.library")} className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-border-subtle px-5 py-3">
        <span className="text-sm tabular-nums text-text-secondary">
          {selectMode
            ? t("library.selectionCount", { count: selected.size })
            : t("shell.status.counts", { count: items.length })}
        </span>
        <div className="flex shrink-0 items-center gap-2">
          {selectMode ? (
            <>
              <Button variant="ghost" size="sm" onClick={toggleAll} aria-pressed={allSelected}>
                {allSelected ? t("library.clearSelection") : t("library.selectAll")}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={exitSelect}
                aria-label={t("library.exitSelect")}
              >
                <X size={14} aria-hidden="true" />
                {t("library.exitSelect")}
              </Button>
            </>
          ) : (
            <>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setSelectMode(true)}
                aria-label={t("library.select")}
              >
                <CheckSquare size={14} aria-hidden="true" />
                {t("library.select")}
              </Button>
              <ImportFileTrigger />
              <AddTitleTrigger />
              <LibraryViewSwitcher view={view} onChange={setView} />
            </>
          )}
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
        <>
          <VirtualizedLibrary
            view={view}
            items={items}
            groupBy={groupBy}
            selectable={selectMode}
            selected={selected}
            onToggle={toggleItem}
          />
          {selectMode && selected.size > 0 ? (
            <BulkActionBar
              ids={[...selected]}
              filter={hasActiveFilters ? filters : null}
              matchingCount={items.length}
              onDone={exitSelect}
            />
          ) : null}
        </>
      )}
    </section>
  );
}
