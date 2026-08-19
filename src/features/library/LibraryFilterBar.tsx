import { useState } from "react";
import { ArrowDownUp, BookmarkPlus, Check, Group, SlidersHorizontal, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
  InputField,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import { CONTENT_TYPE_ORDER, LIBRARY_GROUP_BY, type LibraryGroupBy } from "./grouping";
import type { MediaFacets } from "./api";
import { useCreateSmartCollection } from "@/features/collections/api";
import { toSmartFilter } from "@/features/collections/smartFilter";
import { useToast } from "@/components/ui";
import {
  activeFilterCount,
  DEFAULT_SORT,
  SORT_FIELDS,
  type LibraryFilters,
  type LibrarySort,
  type LibrarySortField,
} from "./filters";
import type { MediaListItem } from "./api";

/* MISSION-041 — Library toolbar: filter panel (type, format, status, genre,
   tag, year, favorite), sort menu (title / added / updated / release year,
   asc/desc) and group-by (status / type / year). Facet options come from the
   `media_facets` endpoint so the panel only offers values that exist.
   MISSION-077 — "Save as collection" snapshots the active filters + sort into
   a smart collection. */

interface LibraryFilterBarProps {
  filters: LibraryFilters;
  sort: LibrarySort;
  groupBy: LibraryGroupBy;
  facets?: MediaFacets;
  onFiltersChange: (filters: LibraryFilters) => void;
  onSortChange: (sort: LibrarySort) => void;
  onGroupByChange: (groupBy: LibraryGroupBy) => void;
}

function OptionChip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-1 rounded-full border border-border-subtle bg-bg-surface px-2.5 py-1 text-sm transition-colors duration-150 ease-out hover:bg-bg-hover",
        active && "border-accent bg-accent text-bg-surface hover:bg-accent",
      )}
    >
      {active && <Check size={12} aria-hidden="true" />}
      {label}
    </button>
  );
}

function FacetSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-text-secondary">
        {title}
      </h3>
      <div className="flex flex-wrap gap-1.5">{children}</div>
    </div>
  );
}

function FiltersPopover({
  filters,
  facets,
  onChange,
}: {
  filters: LibraryFilters;
  facets?: MediaFacets;
  onChange: (filters: LibraryFilters) => void;
}) {
  const { t } = useTranslation();
  const set = (patch: Partial<LibraryFilters>) => onChange({ ...filters, ...patch });
  const clearAll = () =>
    onChange({
      ...filters,
      content_type: null,
      format: null,
      pub_status: null,
      genre: null,
      tag: null,
      year: null,
      favorite: null,
    });

  const option = (active: boolean, onClick: () => void, label: string) => (
    <OptionChip label={label} active={active} onClick={onClick} />
  );

  return (
    <div className="flex w-72 flex-col gap-4">
      {activeFilterCount(filters) > 0 && (
        <div className="flex justify-end">
          <Button variant="ghost" size="sm" onClick={clearAll} className="h-7 px-2 text-xs">
            <X size={12} aria-hidden="true" />
            {t("library.filtersClear")}
          </Button>
        </div>
      )}

      <FacetSection title={t("library.filterType")}>
        {CONTENT_TYPE_ORDER.map((type) =>
          option(
            filters.content_type === type,
            () => set({ content_type: filters.content_type === type ? null : type }),
            t(`contentType.${type}`),
          ),
        )}
      </FacetSection>

      {facets && facets.formats.length > 0 && (
        <FacetSection title={t("library.filterFormat")}>
          {facets.formats.map((format) =>
            option(
              filters.format === format,
              () => set({ format: filters.format === format ? null : format }),
              format,
            ),
          )}
        </FacetSection>
      )}

      <FacetSection title={t("library.filterStatus")}>
        {(["announced", "ongoing", "completed", "hiatus", "cancelled", "unknown"] as const).map(
          (status) =>
            option(
              filters.pub_status === status,
              () => set({ pub_status: filters.pub_status === status ? null : status }),
              t(`pubStatus.${status}`),
            ),
        )}
      </FacetSection>

      {facets && facets.genres.length > 0 && (
        <FacetSection title={t("library.filterGenre")}>
          {facets.genres.map((genre) =>
            option(
              filters.genre === genre.id,
              () => set({ genre: filters.genre === genre.id ? null : genre.id }),
              genre.name,
            ),
          )}
        </FacetSection>
      )}

      {facets && facets.tags.length > 0 && (
        <FacetSection title={t("library.filterTag")}>
          {facets.tags.map((tag) =>
            option(
              filters.tag === tag.id,
              () => set({ tag: filters.tag === tag.id ? null : tag.id }),
              tag.name,
            ),
          )}
        </FacetSection>
      )}

      {facets && facets.years.length > 0 && (
        <FacetSection title={t("library.filterYear")}>
          {facets.years.map((year) =>
            option(
              filters.year === year,
              () => set({ year: filters.year === year ? null : year }),
              String(year),
            ),
          )}
        </FacetSection>
      )}

      <FacetSection title={t("library.filterFavorite")}>
        {option(
          filters.favorite === true,
          () => set({ favorite: filters.favorite === true ? null : true }),
          t("library.favoriteOnly"),
        )}
      </FacetSection>
    </div>
  );
}

const SORT_KEYS: Record<LibrarySortField, string> = {
  title: "library.sortTitle",
  created_at: "library.sortAdded",
  updated_at: "library.sortUpdated",
  release_year: "library.sortRelease",
};

function SortMenu({
  sort,
  onChange,
}: {
  sort: LibrarySort;
  onChange: (sort: LibrarySort) => void;
}) {
  const { t } = useTranslation();
  const pick = (field: LibrarySortField) => {
    if (sort.field === field) {
      onChange({ field, ascending: !sort.ascending });
    } else {
      onChange({ field, ascending: true });
    }
  };
  return (
    <div className="flex w-52 flex-col gap-1">
      {SORT_FIELDS.map((field) => {
        const active = sort.field === field;
        return (
          <button
            key={field}
            type="button"
            aria-pressed={active}
            onClick={() => pick(field)}
            className="flex items-center justify-between rounded-sm px-2 py-1.5 text-sm transition-colors duration-150 ease-out hover:bg-bg-hover"
          >
            <span>{t(SORT_KEYS[field])}</span>
            {active && (
              <span className="text-xs text-text-secondary">
                {t(sort.ascending ? "library.sortAscending" : "library.sortDescending")}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}

const GROUP_KEYS: Record<LibraryGroupBy, string> = {
  none: "library.groupNone",
  content_type: "library.groupType",
  pub_status: "library.groupStatus",
  year: "library.groupYear",
};

function GroupMenu({
  groupBy,
  onChange,
}: {
  groupBy: LibraryGroupBy;
  onChange: (groupBy: LibraryGroupBy) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex w-52 flex-col gap-1">
      {LIBRARY_GROUP_BY.map((value) => {
        const active = groupBy === value;
        return (
          <button
            key={value}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(value)}
            className="flex items-center justify-between rounded-sm px-2 py-1.5 text-sm transition-colors duration-150 ease-out hover:bg-bg-hover"
          >
            <span>{t(GROUP_KEYS[value])}</span>
            {active && <Check size={14} aria-hidden="true" className="text-accent" />}
          </button>
        );
      })}
    </div>
  );
}

export function LibraryFilterBar({
  filters,
  sort,
  groupBy,
  facets,
  onFiltersChange,
  onSortChange,
  onGroupByChange,
}: LibraryFilterBarProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const createSmart = useCreateSmartCollection();
  const filterCount = activeFilterCount(filters);
  const [saveOpen, setSaveOpen] = useState(false);
  const [saveName, setSaveName] = useState("");

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border-subtle px-5 py-2">
      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant="secondary"
            size="sm"
            aria-label={t("library.filters")}
            className={cn(
              "h-[var(--control-height-compact)] px-3 text-sm",
              filterCount > 0 && "border-accent text-accent",
            )}
          >
            <SlidersHorizontal size={14} aria-hidden="true" />
            {t("library.filters")}
            {filterCount > 0 && (
              <span className="inline-flex size-4 items-center justify-center rounded-full bg-accent text-[11px] font-semibold text-bg-surface">
                {filterCount}
              </span>
            )}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-auto">
          <FiltersPopover filters={filters} facets={facets} onChange={onFiltersChange} />
        </PopoverContent>
      </Popover>

      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant="secondary"
            size="sm"
            aria-label={t("library.sort")}
            className={cn(
              "h-[var(--control-height-compact)] px-3 text-sm",
              sort.field !== DEFAULT_SORT.field && "border-accent text-accent",
            )}
          >
            <ArrowDownUp size={14} aria-hidden="true" />
            {t(SORT_KEYS[sort.field])}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-auto">
          <SortMenu sort={sort} onChange={onSortChange} />
        </PopoverContent>
      </Popover>

      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant="secondary"
            size="sm"
            aria-label={t("library.groupBy")}
            className="h-[var(--control-height-compact)] px-3 text-sm"
          >
            <Group size={14} aria-hidden="true" />
            {t(GROUP_KEYS[groupBy])}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-auto">
          <GroupMenu groupBy={groupBy} onChange={onGroupByChange} />
        </PopoverContent>
      </Popover>

      {filterCount > 0 && (
        <Dialog open={saveOpen} onOpenChange={setSaveOpen}>
          <DialogTrigger asChild>
            <Button
              variant="secondary"
              size="sm"
              aria-label={t("collections.saveAsCollection")}
              className="h-[var(--control-height-compact)] px-3 text-sm"
            >
              <BookmarkPlus size={14} aria-hidden="true" />
              {t("collections.saveAsCollection")}
            </Button>
          </DialogTrigger>
          <DialogContent closeLabel={t("a11y.close")}>
            <DialogTitle>{t("collections.saveAsCollectionDialogTitle")}</DialogTitle>
            <DialogDescription>{t("collections.saveAsCollectionDialogHint")}</DialogDescription>
            <form
              onSubmit={(event) => {
                event.preventDefault();
                const trimmed = saveName.trim();
                if (!trimmed) return;
                createSmart.mutate(
                  { name: trimmed, filter: toSmartFilter(filters, sort) },
                  {
                    onSuccess: (view) => {
                      setSaveOpen(false);
                      setSaveName("");
                      toast.success({
                        title: t("collections.smartCreatedToast", { name: view.name }),
                      });
                    },
                    onError: () => toast.error({ title: t("collections.createSmartError") }),
                  },
                );
              }}
              className="mt-4 flex flex-col gap-4"
            >
              <InputField
                label={t("collections.fieldName")}
                placeholder={t("collections.namePlaceholder")}
                value={saveName}
                onChange={(event) => setSaveName(event.target.value)}
              />
              <div className="flex justify-end gap-2">
                <DialogClose asChild>
                  <Button variant="ghost" size="sm" onClick={() => setSaveName("")}>
                    {t("collections.cancel")}
                  </Button>
                </DialogClose>
                <Button
                  type="submit"
                  size="sm"
                  disabled={!saveName.trim() || createSmart.isPending}
                >
                  {t("collections.saveAsCollectionSubmit")}
                </Button>
              </div>
            </form>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}

export type { LibraryFilterBarProps };
export type { MediaListItem };
