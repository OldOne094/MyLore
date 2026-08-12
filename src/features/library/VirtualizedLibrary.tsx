import { useEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "@/lib/cn";
import type { MediaListItem } from "./api";
import { MediaCard } from "./MediaCard";
import { MediaRow } from "./MediaRow";
import type { LibraryView } from "./LibraryViewSwitcher";

/* MISSION-040 — Virtualized library rendering. Three densities: Grid (2:3
   poster cards), List (comfortable rows), Compact (dense rows). Every view
   windows through @tanstack/react-virtual against the local scroll container,
   so 10,000+ entries render without jank (REQ-PERF library line). */

/* Matches the Tailwind grid breakpoints: <640 → 2, ≥640 → 3, ≥768 → 4, ≥1024 → 6. */
const GRID_BREAKPOINTS: ReadonlyArray<{ minWidth: number; columns: number }> = [
  { minWidth: 1024, columns: 6 },
  { minWidth: 768, columns: 4 },
  { minWidth: 640, columns: 3 },
];

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
  className?: string;
}

export function VirtualizedLibrary({ view, items, className }: VirtualizedLibraryProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const columns = useGridColumns(scrollRef);

  const rowCount = view === "grid" ? Math.ceil(items.length / columns) : items.length;
  const rowSize = view === "grid" ? 320 : view === "compact" ? 40 : 64;

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowSize,
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
          if (view === "grid") {
            const start = row.index * columns;
            const rowItems = items.slice(start, start + columns);
            return (
              <div
                key={row.key}
                ref={virtualizer.measureElement}
                data-index={row.index}
                className="absolute inset-x-0 top-0 grid gap-4 px-6 pb-6 pt-6"
                style={{
                  gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
                  transform: `translateY(${row.start}px)`,
                }}
              >
                {rowItems.map((item) => (
                  <MediaCard key={item.id} item={item} />
                ))}
              </div>
            );
          }

          const item = items[row.index];
          if (!item) return null;
          return (
            <div
              key={row.key}
              ref={virtualizer.measureElement}
              data-index={row.index}
              className={cn("absolute inset-x-0 top-0 px-6", view === "compact" ? "pb-1" : "pb-2")}
              style={{ transform: `translateY(${row.start}px)` }}
            >
              <MediaRow item={item} dense={view === "compact"} />
            </div>
          );
        })}
      </div>
    </div>
  );
}
