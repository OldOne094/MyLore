import { cn } from "@/lib/cn";

/* MISSION-049 — Thin accent bar showing completion percent for the in-grid
   quick controls; hidden until there is progress. */

export function ProgressBar({
  percent,
  className,
}: {
  percent: number | null;
  className?: string;
}) {
  if (percent === null || percent <= 0) return null;
  const width = Math.min(100, Math.max(0, percent));
  return (
    <div
      aria-hidden="true"
      className={cn("h-1 overflow-hidden rounded-full bg-accent/20", className)}
    >
      <div className="h-full rounded-full bg-accent" style={{ width: `${width}%` }} />
    </div>
  );
}
