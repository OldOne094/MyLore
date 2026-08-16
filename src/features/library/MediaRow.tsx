import { Check } from "lucide-react";
import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui";
import { cn } from "@/lib/cn";
import type { AssetView } from "@/api";
import type { MediaListItem } from "./api";
import { STATUS_VARIANTS } from "./mediaMeta";
import { CoverImage } from "./CoverImage";
import { NextUnitButton } from "./NextUnitButton";

/* MISSION-040 — Library list rows. `dense` is the Compact tier (DESIGN_SYSTEM
   §8): smaller paddings (8→4), 13px text, hidden meta badges, smaller thumb.
   MISSION-062 renders real cover art in the thumb when the parent passes a
   resolved `cover` view (broken/missing URLs keep the placeholder icon).
   MISSION-042 makes the row a link to the media detail page. MISSION-045 adds
   bulk-select mode: when `selectable` the row becomes a toggle button with a
   leading checkbox instead of a navigation link. MISSION-049 overlays the
   icon-only next-unit control at the trailing edge. */

export interface MediaRowProps {
  item: MediaListItem;
  /** Resolved cover asset view for the thumb (MISSION-062). */
  cover?: AssetView | null;
  dense?: boolean;
  selectable?: boolean;
  selected?: boolean;
  onToggle?: (id: string) => void;
}

export function MediaRow({
  item,
  cover,
  dense = false,
  selectable = false,
  selected = false,
  onToggle,
}: MediaRowProps) {
  const { t } = useTranslation();

  const checkbox = selectable ? (
    <span
      aria-hidden="true"
      className={cn(
        "flex size-4 shrink-0 items-center justify-center rounded-sm border",
        selected ? "border-accent bg-accent text-bg-surface" : "border-border-strong bg-bg-surface",
      )}
    >
      {selected && <Check size={12} />}
    </span>
  ) : null;

  const body = (
    <>
      <div
        className={cn(
          "flex shrink-0 items-center justify-center overflow-hidden rounded-sm bg-bg-hover text-text-tertiary",
          dense ? "size-7" : "size-10",
        )}
      >
        <CoverImage
          asset={cover}
          contentType={item.content_type}
          alt={item.title}
          iconSize={dense ? 14 : 18}
        />
      </div>

      <h3
        className={cn(
          "min-w-0 flex-1 truncate font-medium text-text-primary",
          dense ? "text-xs" : "text-sm",
        )}
      >
        {item.title}
      </h3>

      <Badge variant="accent" className={cn(dense && "hidden")}>
        {t(`contentType.${item.content_type}`)}
      </Badge>
      <Badge
        variant={STATUS_VARIANTS[item.pub_status] ?? "neutral"}
        className={cn(dense && "hidden")}
      >
        {t(`pubStatus.${item.pub_status}`)}
      </Badge>

      {item.release_year ? (
        <span className="shrink-0 text-xs tabular-nums text-text-tertiary">
          {item.release_year}
        </span>
      ) : null}
    </>
  );

  if (selectable) {
    return (
      <button
        type="button"
        aria-pressed={selected}
        aria-label={item.title}
        onClick={() => onToggle?.(item.id)}
        className={cn(
          "flex w-full items-center gap-3 rounded-md border bg-bg-surface transition-colors duration-150 ease-out hover:border-border-subtle hover:bg-bg-hover",
          dense ? "px-2 py-1" : "px-3 py-2",
          selected ? "border-accent ring-1 ring-accent" : "border-transparent",
        )}
      >
        {checkbox}
        {body}
      </button>
    );
  }

  const hasNext = Boolean(item.progress.next_node_id);

  return (
    <div className={cn("relative", dense ? "px-2 py-1" : "px-3 py-2")}>
      <Link
        to={`/library/${item.id}`}
        aria-label={item.title}
        className={cn(
          "flex w-full items-center gap-3 rounded-md border border-transparent bg-bg-surface transition-colors duration-150 ease-out hover:border-border-subtle hover:bg-bg-hover",
          dense ? "px-2 py-1" : "px-3 py-2",
          hasNext && "pe-10",
        )}
      >
        {body}
      </Link>
      <NextUnitButton item={item} dense={dense} className="end-3 top-1/2 -translate-y-1/2" />
    </div>
  );
}
