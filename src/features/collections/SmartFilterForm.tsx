import { ArrowDownUp } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { SmartFilter } from "@/api";
import type { MediaFacets } from "@/features/library/api";
import { CONTENT_TYPE_ORDER } from "@/features/library/grouping";
import { SORT_FIELDS, type LibrarySortField } from "@/features/library/filters";

/* MISSION-077 — Basic query builder for smart collections. One nullable facet
   per filter field (matching the library filter panel) plus the sort field and
   direction. Facet options come from `media_facets` when available, so the
   builder only offers values that exist in the library. */

const SORT_KEYS: Record<LibrarySortField, string> = {
  title: "library.sortTitle",
  created_at: "library.sortAdded",
  updated_at: "library.sortUpdated",
  release_year: "library.sortRelease",
};

const PUB_STATUSES = [
  "announced",
  "ongoing",
  "completed",
  "hiatus",
  "cancelled",
  "unknown",
] as const;

interface SmartFilterFormProps {
  value: SmartFilter;
  onChange: (filter: SmartFilter) => void;
  facets?: MediaFacets;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-text-secondary">
      {label}
      {children}
    </label>
  );
}

const selectClass =
  "h-[var(--control-height-compact)] w-full rounded-md border border-border-subtle bg-bg-surface px-2 text-sm text-text-primary outline-none transition-colors duration-150 ease-out hover:border-border-strong focus:border-accent";

export function SmartFilterForm({ value, onChange, facets }: SmartFilterFormProps) {
  const { t } = useTranslation();

  const set = (patch: Partial<SmartFilter>) => onChange({ ...value, ...patch });
  const any = (key: string) => <option value="">{t(key)}</option>;

  return (
    <div className="grid grid-cols-2 gap-3">
      <Field label={t("library.filterType")}>
        <select
          className={selectClass}
          value={value.content_type ?? ""}
          onChange={(event) => set({ content_type: event.target.value || null })}
        >
          {any("collections.filterAny")}
          {CONTENT_TYPE_ORDER.map((type) => (
            <option key={type} value={type}>
              {t(`contentType.${type}`)}
            </option>
          ))}
        </select>
      </Field>

      <Field label={t("library.filterFormat")}>
        <select
          className={selectClass}
          value={value.format ?? ""}
          onChange={(event) => set({ format: event.target.value || null })}
        >
          {any("collections.filterAny")}
          {(facets?.formats ?? []).map((format) => (
            <option key={format} value={format}>
              {format}
            </option>
          ))}
        </select>
      </Field>

      <Field label={t("library.filterStatus")}>
        <select
          className={selectClass}
          value={value.pub_status ?? ""}
          onChange={(event) => set({ pub_status: event.target.value || null })}
        >
          {any("collections.filterAny")}
          {PUB_STATUSES.map((status) => (
            <option key={status} value={status}>
              {t(`pubStatus.${status}`)}
            </option>
          ))}
        </select>
      </Field>

      <Field label={t("library.filterGenre")}>
        <select
          className={selectClass}
          value={value.genre ?? ""}
          onChange={(event) => set({ genre: event.target.value || null })}
        >
          {any("collections.filterAny")}
          {(facets?.genres ?? []).map((genre) => (
            <option key={genre.id} value={genre.id}>
              {genre.name}
            </option>
          ))}
        </select>
      </Field>

      <Field label={t("library.filterTag")}>
        <select
          className={selectClass}
          value={value.tag ?? ""}
          onChange={(event) => set({ tag: event.target.value || null })}
        >
          {any("collections.filterAny")}
          {(facets?.tags ?? []).map((tag) => (
            <option key={tag.id} value={tag.id}>
              {tag.name}
            </option>
          ))}
        </select>
      </Field>

      <Field label={t("library.filterYear")}>
        <select
          className={selectClass}
          value={value.year === null ? "" : String(value.year)}
          onChange={(event) =>
            set({ year: event.target.value === "" ? null : Number(event.target.value) })
          }
        >
          {any("collections.filterAny")}
          {(facets?.years ?? []).map((year) => (
            <option key={year} value={year}>
              {year}
            </option>
          ))}
        </select>
      </Field>

      <Field label={t("library.filterFavorite")}>
        <select
          className={selectClass}
          value={value.favorite === null ? "" : value.favorite ? "true" : "false"}
          onChange={(event) =>
            set({
              favorite: event.target.value === "" ? null : event.target.value === "true",
            })
          }
        >
          {any("collections.filterAny")}
          <option value="true">{t("library.favoriteOnly")}</option>
        </select>
      </Field>

      <Field label={t("collections.sortField")}>
        <select
          className={selectClass}
          value={value.sort ?? ""}
          onChange={(event) =>
            set({
              sort: event.target.value || null,
              ascending: event.target.value ? (value.ascending ?? true) : null,
            })
          }
        >
          {any("collections.filterAny")}
          {SORT_FIELDS.map((field) => (
            <option key={field} value={field}>
              {t(SORT_KEYS[field])}
            </option>
          ))}
        </select>
      </Field>

      <div className="col-span-2 flex items-center justify-end">
        <button
          type="button"
          disabled={!value.sort}
          onClick={() => set({ ascending: !(value.ascending ?? true) })}
          className="inline-flex items-center gap-1.5 rounded-md border border-border-subtle bg-bg-surface px-2.5 py-1.5 text-xs text-text-secondary transition-colors duration-150 ease-out hover:border-border-strong hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-50"
        >
          <ArrowDownUp size={12} aria-hidden="true" />
          {t((value.ascending ?? true) ? "library.sortAscending" : "library.sortDescending")}
        </button>
      </div>
    </div>
  );
}

export type { SmartFilterFormProps };
