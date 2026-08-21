import * as DialogPrimitive from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Dialog (Radix): focus trap, Esc close, dir aware.
   Cards: dialog lg radius, raised surface, elevation-lg. */

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export interface DialogContentProps extends ComponentPropsWithoutRef<
  typeof DialogPrimitive.Content
> {
  children: ReactNode;
  /** Drop the default padding (e.g. for a command palette's flush layout). */
  noPadding?: boolean;
  /** Accessible name for the close button (defaults to "Close"). */
  closeLabel?: string;
}

export function DialogContent({
  className,
  children,
  noPadding,
  closeLabel = "Close",
  ...props
}: DialogContentProps) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm" />
      <DialogPrimitive.Content
        className={cn(
          "fixed left-1/2 top-1/2 z-50 w-[calc(100%-2rem)] max-w-lg",
          "max-h-[85vh] overflow-y-auto",
          "translate-x-[-50%] translate-y-[-50%]",
          "rounded-lg border border-border-subtle bg-bg-raised shadow-lg",
          !noPadding && "p-6",
          "outline-none",
          className,
        )}
        {...props}
      >
        {children}
        <DialogPrimitive.Close
          aria-label={closeLabel}
          className="absolute end-4 top-4 flex size-8 items-center justify-center rounded-sm text-text-tertiary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary"
        >
          <X size={16} aria-hidden="true" />
        </DialogPrimitive.Close>
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
}

export function DialogTitle({
  className,
  ...props
}: ComponentPropsWithoutRef<typeof DialogPrimitive.Title>) {
  return (
    <DialogPrimitive.Title
      className={cn("text-lg font-semibold text-text-primary", className)}
      {...props}
    />
  );
}

export function DialogDescription({
  className,
  ...props
}: ComponentPropsWithoutRef<typeof DialogPrimitive.Description>) {
  return (
    <DialogPrimitive.Description
      className={cn("mt-1 text-sm text-text-secondary", className)}
      {...props}
    />
  );
}
