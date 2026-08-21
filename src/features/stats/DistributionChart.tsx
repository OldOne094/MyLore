import type { StatCount } from "@/api";

/* MISSION-080 — Horizontal distribution chart shared by the Stats page and the
   reading recap (MISSION-083): a labelled row per non-zero bucket with an
   accent progress bar scaled to the largest bucket. Hand-rolled with logical
   properties so it stays RTL-safe and dependency-light. */

export function DistributionChart({
  title,
  rows,
  format,
  emptyLabel,
}: {
  title: string;
  rows: StatCount[];
  format: (key: string) => string;
  emptyLabel: string;
}) {
  const visible = rows.filter((row) => row.count > 0);
  const max = Math.max(1, ...visible.map((row) => row.count));
  return (
    <section className="rounded-md border border-border-subtle bg-bg-surface p-4">
      <h2 className="text-sm font-semibold text-text-primary">{title}</h2>
      {visible.length === 0 ? (
        <p className="mt-2 text-sm text-text-tertiary">{emptyLabel}</p>
      ) : (
        <ul className="mt-3 flex flex-col gap-2">
          {visible.map((row) => (
            <li key={row.key} className="flex items-center gap-2">
              <span className="w-24 shrink-0 truncate text-xs text-text-secondary">
                {format(row.key)}
              </span>
              <div
                className="h-2 flex-1 overflow-hidden rounded-full bg-accent/20"
                aria-hidden="true"
              >
                <div
                  className="h-full rounded-full bg-accent transition-[width] duration-150 ease-out"
                  style={{ width: `${(row.count / max) * 100}%` }}
                />
              </div>
              <span className="w-8 shrink-0 text-end text-xs tabular-nums text-text-tertiary">
                {row.count}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export type { StatCount };
