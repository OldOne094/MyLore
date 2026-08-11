import { Slot } from "@radix-ui/react-slot";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Button. Variants: primary / secondary / ghost / danger.
   Sizes: sm / md. Icon-only buttons set aria-label themselves; `asChild` composes
   (e.g. a button wrapping a Link or a Radix trigger). */

const VARIANT_CLASSES = {
  primary:
    "bg-accent text-bg-surface border border-accent hover:bg-accent-hover hover:border-accent-hover",
  secondary: "bg-bg-surface text-text-primary border border-border-strong hover:bg-bg-hover",
  ghost: "bg-transparent text-text-primary border border-transparent hover:bg-bg-hover",
  danger: "bg-danger text-bg-surface border border-danger hover:opacity-90",
} as const;

const SIZE_CLASSES = {
  sm: "h-[var(--control-height-compact)] px-3 text-sm",
  md: "h-[var(--control-height)] px-4 text-base",
} as const;

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: keyof typeof VARIANT_CLASSES;
  size?: keyof typeof SIZE_CLASSES;
  /** Compose the button around a child element (Radix trigger, link). */
  asChild?: boolean;
  children: ReactNode;
}

export function Button({
  variant = "primary",
  size = "md",
  asChild = false,
  className,
  type = "button",
  children,
  ...props
}: ButtonProps) {
  const classes = cn(
    "inline-flex select-none items-center justify-center gap-2 whitespace-nowrap rounded-sm font-medium",
    "transition-colors duration-150 ease-out disabled:pointer-events-none disabled:opacity-50",
    "data-[state=open]:bg-bg-hover",
    VARIANT_CLASSES[variant],
    SIZE_CLASSES[size],
    className,
  );

  const Comp = asChild ? Slot : "button";
  return (
    <Comp className={classes} type={asChild ? undefined : type} {...props}>
      {children}
    </Comp>
  );
}
