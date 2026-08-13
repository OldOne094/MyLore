import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "@/lib/cn";
import type { MediaListItem } from "./api";
import { MediaCard } from "./MediaCard";
import { MediaRow } from "./MediaRow";
import type { LibraryView } from "./LibraryViewSwitcher";
import { buildLibraryRows, type LibraryGroupBy, type LibraryRow } from "./grouping";

/* MISSION-040/041 — Virtualized library rendering. Three densities: Grid (2:3
   poster cards), List (comfortable rows), Compact (dense rows). Every view
   windows through @tanstack/react-virtual against the local scroll container,
   so 10,000+ entries render without jank (REQ-PERF library line). MISSION-041
   adds group-by: the flat row model interleaves group-header rows with item
   rows, all virtualized together. */

/* Matches the Tailwind grid breakpoints: <640 → 2, ≥640 → 3, ≥768 → 4, ≥1024 → 6. */
const GRID_BREAKPOINTS: ReadonlyArray<{ minWidth: number; columns: number }> = [
  { minWidth: 1024, columns: 6 },
  { minWidth: 768, columns: 4 },
  { minWidth: 640, columns: 3 },
];

const HEADER_SIZE = 36;

function columnsForWidth(width: number): number {
  for (const breakpoint of GRID_BREAKPOINTS) {
    if (width >= breakpoint.minWidth) return breakpoint.columns;
  }
  return 2;
}

function useGridColumns(scrollRef: React.RefObject<HTMLDivElement | null>): number {
  const [columns, setColumns] = useState(() => columnsForWidth(window.innerWidth));

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    const update = () => setColumns(columnsForWidth(element.clientWidth));
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, [scrollRef]);

  return columns;
}

export interface VirtualizedLibraryProps {
  view: LibraryView;
  items: MediaListItem[];
  groupBy?: LibraryGroupBy;
  /** Bulk-select mode: cards/rows render as toggles instead of links (MISSION-045). */
  selectable?: boolean;
  selected?: ReadonlySet<string>;
  onToggle?: (id: string) => void;
  className?: string;
}

export function VirtualizedLibrary({
  view,
  items,
  groupBy = "none",
  selectable = false,
  selected,
  onToggle,
  className,
}: VirtualizedLibraryProps) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);
  const columns = useGridColumns(scrollRef);

  const rowSize = view === "grid" ? 320 : view === "compact" ? 40 : 64;

  const rows: LibraryRow[] = useMemo(() => {
    const labelFor = (group: LibraryGroupBy, raw: string): string => {
      if (group === "year" && raw === "unknown") return t("library.groupUnknown");
      if (group === "content_type") return t(`contentType.${raw}`);
      if (group === "pub_status") return t(`pubStatus.${raw}`);
      return raw;
    };
    const itemColumns = view === "grid" ? columns : 1;
    return buildLibraryRows(items, groupBy, itemColumns, labelFor);
  }, [items, groupBy, view, columns, t]);

  const rowsRef = useRef(rows);
  rowsRef.current = rows;

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) => (rowsRef.current[index]?.kind === "header" ? HEADER_SIZE : rowSize),
    overscan: view === "grid" ? 2 : 8,
  });

  return (
    <div
      ref={scrollRef}
      role="list"
      aria-label="Library"
      className={cn("min-h-0 flex-1 overflow-auto", className)}
    >
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((row) => {
          const entry = rows[row.index];
          if (entry.kind === "header") {
            return (
              <div
                key={entry.key}
                ref={virtualizer.measureElement}
                data-index={row.index}
                className="absolute inset-x-0 top-0 flex items-center px-6"
                style={{ height: HEADER_SIZE, transform: `translateY(${row.start}px)` }}
              >
                <span className="text-xs font-semibold uppercase tracking-wider text-text-secondary">
                  {entry.label}
                </span>
              </div>
            );
          }

          if (view === "grid") {
            return (
              <div
                key={entry.key}
                ref={virtualizer.measureElement}
                data-index={row.index}
                className="absolute inset-x-0 top-0 grid gap-4 px-6 pb-6 pt-6"
                style={{
                  gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
                  transform: `translateY(${row.start}px)`,
                }}
              >
                {entry.items.map((item) => (
                  <MediaCard
                    key={item.id}
                    item={item}
                    selectable={selectable}
                    selected={selected?.has(item.id) ?? false}
                    onToggle={onToggle}
                  />
                ))}
              </div>
            );
          }

          const item = entry.items[0];
          if (!item) return null;
          return (
            <div
              key={entry.key}
              ref={virtualizer.measureElement}
              data-index={row.index}
              className={cn("absolute inset-x-0 top-0 px-6", view === "compact" ? "pb-1" : "pb-2")}
              style={{ transform: `translateY(${row.start}px)` }}
            >
              <MediaRow
                item={item}
                dense={view === "compact"}
                selectable={selectable}
                selected={selected?.has(item.id) ?? false}
                onToggle={onToggle}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
