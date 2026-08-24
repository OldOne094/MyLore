import {
  BookOpen,
  Clapperboard,
  Gamepad2,
  Mic,
  Music2,
  BookMarked,
  Sparkles,
  Tv,
  type LucideIcon,
} from "lucide-react";
import type { BadgeVariant } from "@/components/ui";

/* MISSION-040/109 — Shared card/row metadata mapping. Content-type icons and
   publication-status badge variants used by every library density so Grid,
   List and Compact views never drift apart. */

export const TYPE_ICONS: Record<string, LucideIcon> = {
  book: BookOpen,
  novel: BookOpen,
  web_novel: BookOpen,
  manga: BookOpen,
  manhwa: BookOpen,
  manhua: BookOpen,
  comic: BookMarked,
  anime: Tv,
  tv: Tv,
  movie: Clapperboard,
  game: Gamepad2,
  podcast: Mic,
  music: Music2,
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
