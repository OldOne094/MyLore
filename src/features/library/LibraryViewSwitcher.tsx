import { LayoutGrid, List, Rows3, type LucideIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/cn";

/* MISSION-040 — Library view switcher (Grid / List / Compact list). Segmented
   control mirroring the theme/language switchers; buttons are icon-only with
   translated aria-labels (DESIGN_SYSTEM.md §5). */

export type LibraryView = "grid" | "list" | "compact";

const LIBRARY_VIEWS: LibraryView[] = ["grid", "list", "compact"];

const VIEW_ICONS: Record<LibraryView, LucideIcon> = {
  grid: LayoutGrid,
  list: List,
  compact: Rows3,
};

const VIEW_LABEL_KEYS: Record<LibraryView, string> = {
  grid: "a11y.viewGrid",
  list: "a11y.viewList",
  compact: "a11y.viewCompact",
};

export interface LibraryViewSwitcherProps {
  view: LibraryView;
  onChange: (view: LibraryView) => void;
}

export function LibraryViewSwitcher({ view, onChange }: LibraryViewSwitcherProps) {
  const { t } = useTranslation();

  return (
    <div
      role="group"
      aria-label={t("a11y.viewSwitcher")}
      className="inline-flex items-center gap-1 rounded-full border border-border-subtle bg-bg-surface p-1"
    >
      {LIBRARY_VIEWS.map((value) => {
        const Icon = VIEW_ICONS[value];
        const active = view === value;
        return (
          <button
            key={value}
            type="button"
            aria-pressed={active}
            aria-label={t(VIEW_LABEL_KEYS[value])}
            title={t(VIEW_LABEL_KEYS[value])}
            onClick={() => onChange(value)}
            className={cn(
              "rounded-full border-none bg-transparent p-1.5 text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary",
              active && "bg-accent text-bg-surface hover:bg-accent",
            )}
          >
            <Icon size={16} aria-hidden="true" />
          </button>
        );
      })}
    </div>
  );
}
