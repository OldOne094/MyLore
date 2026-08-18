import { Check } from "lucide-react";
import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui";
import { cn } from "@/lib/cn";
import type { AssetView } from "@/api";
import type { MediaListItem } from "./api";
import { STATUS_VARIANTS } from "./mediaMeta";
import { CoverImage } from "./CoverImage";
import { FavoriteFlag } from "./FavoriteFlag";
import { NextUnitButton } from "./NextUnitButton";
import { ProgressBar } from "./ProgressBar";

/* MISSION-040 — Single library card in the grid. MISSION-062 renders real cover
   art through the asset pipeline when the parent passes a resolved `cover` view
   (cached → `convertFileSrc`; any other status → the placeholder icon, so broken
   URLs never show a broken image). MISSION-042 makes the card a link to the
   media detail page. MISSION-045 adds bulk-select mode: when `selectable` the
   card becomes a toggle button (aria-pressed) with a corner checkbox instead of
   a navigation link. MISSION-049 overlays the next-unit pill and a thin progress
   bar at the poster's bottom edge. */

export interface MediaCardProps {
  item: MediaListItem;
  /** Resolved cover asset view for the poster (MISSION-062). */
  cover?: AssetView | null;
  selectable?: boolean;
  selected?: boolean;
  onToggle?: (id: string) => void;
}

export function MediaCard({
  item,
  cover,
  selectable = false,
  selected = false,
  onToggle,
}: MediaCardProps) {
  const { t } = useTranslation();

  const poster = (
    <div className="relative flex aspect-[2/3] w-full items-center justify-center overflow-hidden rounded-sm bg-bg-hover text-text-tertiary">
      <CoverImage asset={cover} contentType={item.content_type} alt={item.title} iconSize={28} />
      {item.favorite && (
        <FavoriteFlag
          size={13}
          className="absolute start-2 top-2 size-6 rounded-full bg-bg-base/90"
        />
      )}
      <ProgressBar percent={item.progress.percent} className="absolute inset-x-0 bottom-0" />
    </div>
  );

  const body = (
    <>
      {poster}
      <div className="flex flex-col gap-1.5 px-0.5">
        <h3 className="line-clamp-2 text-sm font-medium text-text-primary">{item.title}</h3>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant="accent">{t(`contentType.${item.content_type}`)}</Badge>
          <Badge variant={STATUS_VARIANTS[item.pub_status] ?? "neutral"}>
            {t(`pubStatus.${item.pub_status}`)}
          </Badge>
          {item.release_year ? (
            <span className="text-xs tabular-nums text-text-tertiary">{item.release_year}</span>
          ) : null}
        </div>
      </div>
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
          "relative flex flex-col gap-2 rounded-lg border bg-bg-surface p-2.5 text-start",
          "transition-colors duration-150 ease-out hover:border-border-strong",
          selected && "border-accent ring-1 ring-accent",
        )}
      >
        <span
          aria-hidden="true"
          className={cn(
            "absolute end-2 top-2 flex size-5 items-center justify-center rounded-sm border",
            selected
              ? "border-accent bg-accent text-bg-surface"
              : "border-border-strong bg-bg-surface",
          )}
        >
          {selected && <Check size={14} />}
        </span>
        {body}
      </button>
    );
  }

  return (
    <div className="relative">
      <Link
        to={`/library/${item.id}`}
        aria-label={item.title}
        className="flex flex-col gap-2 rounded-lg border border-border-subtle bg-bg-surface p-2.5 transition-colors duration-150 ease-out hover:border-border-strong"
      >
        {body}
      </Link>
      <NextUnitButton item={item} className="absolute end-2 top-2" />
    </div>
  );
}
