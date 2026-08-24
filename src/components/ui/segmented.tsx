import { cn } from "@/lib/cn";

/* Segmented control — the single implementation behind theme/language/
   density switchers and settings section tabs. One shape, one size
   (h-7 pills in a bordered track), selected = accent pill, and the global
   focus ring handles keyboard visibility. Plain buttons inside a labelled
   group keep every existing test contract intact. */

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
}

export interface SegmentedProps<T extends string> {
  value: T;
  options: readonly SegmentedOption<T>[];
  onChange: (value: T) => void;
  /** Accessible group name. */
  "aria-label": string;
  className?: string;
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  className,
  "aria-label": ariaLabel,
}: SegmentedProps<T>) {
  return (
    <div
      role="group"
      aria-label={ariaLabel}
      className={cn(
        "inline-flex select-none items-center gap-0.5 rounded-full border border-border-subtle bg-bg-base p-1",
        className,
      )}
    >
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(option.value)}
            className={cn(
              "h-7 rounded-full px-3 text-sm font-medium transition-colors duration-150 ease-out",
              active
                ? "bg-accent text-bg-surface"
                : "text-text-secondary hover:bg-bg-hover hover:text-text-primary",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
