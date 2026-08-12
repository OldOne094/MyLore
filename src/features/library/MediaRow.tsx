import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui";
import { cn } from "@/lib/cn";
import type { MediaListItem } from "./api";
import { STATUS_VARIANTS, TYPE_ICONS } from "./mediaMeta";

/* MISSION-040 — Library list rows. `dense` is the Compact tier (DESIGN_SYSTEM
   §8): smaller paddings (8→4), 13px text, hidden meta badges, smaller thumb. */

export interface MediaRowProps {
  item: MediaListItem;
  dense?: boolean;
}

export function MediaRow({ item, dense = false }: MediaRowProps) {
  const { t } = useTranslation();
  const Icon = TYPE_ICONS[item.content_type] ?? TYPE_ICONS.other;

  return (
    <article
      className={cn(
        "flex w-full items-center gap-3 rounded-md border border-transparent bg-bg-surface transition-colors duration-150 ease-out hover:border-border-subtle hover:bg-bg-hover",
        dense ? "px-2 py-1" : "px-3 py-2",
      )}
    >
      <div
        className={cn(
          "flex shrink-0 items-center justify-center overflow-hidden rounded-sm bg-bg-hover text-text-tertiary",
          dense ? "size-7" : "size-10",
        )}
      >
        <Icon size={dense ? 14 : 18} aria-hidden="true" />
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
    </article>
  );
}
