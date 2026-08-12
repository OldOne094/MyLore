import { BookOpen, Clapperboard, Sparkles, Tv, type LucideIcon } from "lucide-react";
import type { BadgeVariant } from "@/components/ui";

/* MISSION-040 — Shared card/row metadata mapping. Content-type icons and
   publication-status badge variants used by every library density so Grid,
   List and Compact views never drift apart. */

export const TYPE_ICONS: Record<string, LucideIcon> = {
  book: BookOpen,
  novel: BookOpen,
  web_novel: BookOpen,
  manga: BookOpen,
  manhwa: BookOpen,
  manhua: BookOpen,
  anime: Tv,
  tv: Tv,
  movie: Clapperboard,
  other: Sparkles,
};

export const STATUS_VARIANTS: Record<string, BadgeVariant> = {
  announced: "planned",
  ongoing: "inprogress",
  completed: "completed",
  hiatus: "onhold",
  cancelled: "dropped",
  unknown: "neutral",
};
