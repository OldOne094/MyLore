import { Heart } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/cn";

/* MISSION-075 — Favorite flag shown in the grid/list views when the media's
   review row carries `favorite`. A pure visual indicator (non-interactive);
   the toggle lives in the detail page's Review tab. */

interface FavoriteFlagProps {
  className?: string;
  /** px size passed to the Heart icon. */
  size?: number;
}

export function FavoriteFlag({ className, size = 13 }: FavoriteFlagProps) {
  const { t } = useTranslation();
  return (
    <span
      role="img"
      aria-label={t("review.favoriteLabel")}
      className={cn("inline-flex shrink-0 items-center justify-center", className)}
    >
      <Heart size={size} className="fill-current text-danger" aria-hidden="true" />
    </span>
  );
}
