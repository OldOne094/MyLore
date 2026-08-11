import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Popover (Radix): keyboard-openable, dir aware.
   Content sits on a raised surface with elevation-lg. */

export const Popover = PopoverPrimitive.Root;
export const PopoverTrigger = PopoverPrimitive.Trigger;
export const PopoverClose = PopoverPrimitive.Close;

export interface PopoverContentProps extends ComponentPropsWithoutRef<
  typeof PopoverPrimitive.Content
> {
  children: ReactNode;
}

export function PopoverContent({ className, children, ...props }: PopoverContentProps) {
  return (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        sideOffset={6}
        className={cn(
          "z-50 rounded-md border border-border-subtle bg-bg-raised p-3 shadow-lg",
          className,
        )}
        {...props}
      >
        {children}
        <PopoverPrimitive.Arrow className="fill-bg-raised" />
      </PopoverPrimitive.Content>
    </PopoverPrimitive.Portal>
  );
}
