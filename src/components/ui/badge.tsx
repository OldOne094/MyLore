import type { HTMLAttributes } from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Badge/Chip: status, genre, tag, external-provider.
   Status variants map to the tokenized status palette. */

export type BadgeVariant =
  "neutral" | "accent" | "planned" | "inprogress" | "completed" | "onhold" | "dropped" | "repeat";

const VARIANT_CLASSES: Record<BadgeVariant, string> = {
  neutral: "text-text-secondary bg-bg-hover border-border-subtle",
  accent: "text-accent bg-accent-soft border-accent/30",
  planned: "text-status-planned bg-status-planned/12 border-status-planned/30",
  inprogress: "text-status-inprogress bg-status-inprogress/12 border-status-inprogress/30",
  completed: "text-status-completed bg-status-completed/12 border-status-completed/30",
  onhold: "text-status-onhold bg-status-onhold/12 border-status-onhold/30",
  dropped: "text-status-dropped bg-status-dropped/12 border-status-dropped/30",
  repeat: "text-status-repeat bg-status-repeat/12 border-status-repeat/30",
};

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
}

export function Badge({ variant = "neutral", className, ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 whitespace-nowrap rounded-full border px-2.5 py-0.5 text-xs font-medium",
        VARIANT_CLASSES[variant],
        className,
      )}
      {...props}
    />
  );
}
