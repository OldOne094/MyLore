import { Play } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useToast } from "@/components/ui";
import { cn } from "@/lib/cn";
import type { MediaListItem } from "./api";
import { consumingStateFor, useMarkNextUnit } from "./progress";

/* MISSION-049 — In-grid quick control. A pill on the card's poster (icon-only
   on the list row) that marks the next unit done in one click. It must render
   as a sibling of the card/row `<Link>` — an interactive element nested inside
   a link is invalid — so it sits absolutely positioned above the link and
   swallows its own clicks. Hidden when there is nothing left to mark or in
   select mode. */

export function NextUnitButton({
  item,
  dense = false,
  className,
}: {
  item: MediaListItem;
  dense?: boolean;
  className?: string;
}) {
  const { t } = useTranslation();
  const toast = useToast();
  const markNext = useMarkNextUnit();

  const { next_node_id, next_label } = item.progress;
  if (!next_node_id || !next_label) return null;

  const watched = consumingStateFor(item.content_type) === "watched";
  const aria = t(watched ? "progress.toggleWatched" : "progress.toggleRead", {
    label: next_label,
  });

  const run = () => {
    markNext.mutate(item.id, {
      onSuccess: (view) => {
        if (!view) toast.info({ title: t("quick.allCaughtUp") });
      },
    });
  };

  return (
    <button
      type="button"
      onClick={run}
      disabled={markNext.isPending}
      aria-label={aria}
      title={aria}
      className={cn(
        "absolute z-10 inline-flex items-center justify-center gap-1 rounded-full border bg-bg-surface/95 shadow-sm transition-colors duration-150 ease-out hover:border-accent hover:text-accent focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent disabled:opacity-60",
        className,
        dense ? "size-6" : "h-6 px-2",
      )}
    >
      {dense ? (
        <Play size={11} aria-hidden="true" />
      ) : (
        <>
          <Play size={10} aria-hidden="true" />
          <span className="text-[11px] font-medium leading-none">{next_label}</span>
        </>
      )}
    </button>
  );
}
