import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui";
import type { MediaListItem } from "./api";
import { STATUS_VARIANTS, TYPE_ICONS } from "./mediaMeta";

/* MISSION-040 — Single library card in the grid. Poster placeholder until real
   cover art arrives (cover_asset_id is unresolved for now). MISSION-042 makes
   the card a link to the media detail page. */

export interface MediaCardProps {
  item: MediaListItem;
}

export function MediaCard({ item }: MediaCardProps) {
  const { t } = useTranslation();
  const Icon = TYPE_ICONS[item.content_type] ?? TYPE_ICONS.other;

  return (
    <Link
      to={`/library/${item.id}`}
      aria-label={item.title}
      className="flex flex-col gap-2 rounded-lg border border-border-subtle bg-bg-surface p-2.5 transition-colors duration-150 ease-out hover:border-border-strong"
    >
      <div className="flex aspect-[2/3] w-full items-center justify-center overflow-hidden rounded-sm bg-bg-hover text-text-tertiary">
        <Icon size={28} aria-hidden="true" />
      </div>
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
    </Link>
  );
}
