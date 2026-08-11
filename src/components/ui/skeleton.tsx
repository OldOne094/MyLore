import type { HTMLAttributes } from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Skeleton placeholder. Shimmer-free (reduced-motion
   disables animation globally); pulsing is subtle and non-decorative. */

export function Skeleton({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("animate-pulse rounded-sm bg-bg-hover", className)}
      aria-hidden="true"
      {...props}
    />
  );
}
