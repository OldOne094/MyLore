import type { ComponentType, ReactNode } from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Empty state: icon + title + hint + primary action.
   Used by every placeholder/empty view until real data lands. */

export interface EmptyStateProps {
  icon: ComponentType<{ size?: number; className?: string }>;
  title: string;
  hint?: string;
  action?: ReactNode;
  className?: string;
}

export function EmptyState({ icon: Icon, title, hint, action, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-2 px-6 py-12 text-center",
        className,
      )}
    >
      <div className="flex size-12 items-center justify-center rounded-full bg-bg-hover text-text-tertiary">
        <Icon size={24} aria-hidden="true" />
      </div>
      <h2 className="mt-1 text-md font-semibold text-text-primary">{title}</h2>
      {hint ? <p className="max-w-sm text-sm text-text-secondary">{hint}</p> : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}
