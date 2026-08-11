import { Label } from "@radix-ui/react-label";
import { useId } from "react";
import type { InputHTMLAttributes } from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Input / Textarea share field tokens. A label is required;
   an inline validation slot shows under the field when `error` is set. */

const FIELD_CLASSES =
  "w-full rounded-sm border bg-bg-base text-text-primary placeholder:text-text-tertiary " +
  "transition-colors duration-150 ease-out hover:border-accent " +
  "focus-visible:outline-none data-[invalid=true]:border-danger";

export interface InputFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string;
  error?: string;
}

export function InputField({ label, error, id, className, ...props }: InputFieldProps) {
  const autoId = useId();
  const fieldId = id ?? autoId;

  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={fieldId} className="text-sm font-medium text-text-secondary">
        {label}
      </Label>
      <input
        id={fieldId}
        data-invalid={Boolean(error)}
        aria-invalid={Boolean(error) || undefined}
        aria-describedby={error ? `${fieldId}-error` : undefined}
        className={cn(FIELD_CLASSES, "h-[var(--control-height)] px-3 text-base", className)}
        {...props}
      />
      {error ? (
        <span id={`${fieldId}-error`} role="alert" className="text-xs text-danger">
          {error}
        </span>
      ) : null}
    </div>
  );
}

export interface TextareaFieldProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label: string;
  error?: string;
}

export function TextareaField({ label, error, id, className, ...props }: TextareaFieldProps) {
  const autoId = useId();
  const fieldId = id ?? autoId;

  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={fieldId} className="text-sm font-medium text-text-secondary">
        {label}
      </Label>
      <textarea
        id={fieldId}
        data-invalid={Boolean(error)}
        aria-invalid={Boolean(error) || undefined}
        aria-describedby={error ? `${fieldId}-error` : undefined}
        className={cn(FIELD_CLASSES, "min-h-24 px-3 py-2 text-base", className)}
        {...props}
      />
      {error ? (
        <span id={`${fieldId}-error`} role="alert" className="text-xs text-danger">
          {error}
        </span>
      ) : null}
    </div>
  );
}
