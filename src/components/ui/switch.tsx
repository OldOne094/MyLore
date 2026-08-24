import { cn } from "@/lib/cn";

/* Settings-grade toggle switch (MISSION-130 polish). Accessible by contract:
   role=switch + aria-checked + a caller-provided label. RTL-safe: the knob
   travels with the reading direction via the rtl: variant instead of a fixed
   physical transform. */

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** Accessible name — switches have no visible label of their own. */
  "aria-label": string;
  disabled?: boolean;
  className?: string;
}

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  className,
  "aria-label": ariaLabel,
}: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border transition-colors duration-150 ease-out",
        "focus-visible:outline-none",
        "disabled:pointer-events-none disabled:opacity-50",
        checked ? "border-accent bg-accent" : "border-border-strong bg-bg-raised",
        className,
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "absolute start-[3px] top-1/2 size-4 -translate-y-1/2 rounded-full bg-bg-surface shadow-sm",
          "transition-transform duration-150 ease-out",
          checked ? "translate-x-[22px] rtl:-translate-x-[22px]" : "translate-x-0",
        )}
      />
    </button>
  );
}
