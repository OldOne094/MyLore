import * as ToastPrimitive from "@radix-ui/react-toast";
import { X } from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Toast: success / error / info, optional undo action,
   auto-dismiss (pause on hover is built into Radix), RTL-aware swipe. */

export type ToastKind = "success" | "error" | "info";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface ToastData {
  id: number;
  kind: ToastKind;
  title: string;
  description?: string;
  action?: ToastAction;
}

const KIND_CLASSES: Record<ToastKind, string> = {
  success: "border-s-status-completed",
  error: "border-s-status-dropped",
  info: "border-s-status-inprogress",
};

interface ToastContextValue {
  push: (data: Omit<ToastData, "id">) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export interface ToastProviderProps {
  children: ReactNode;
  /** Auto-dismiss delay in ms (default 5000). */
  duration?: number;
}

export function ToastProvider({ children, duration = 5000 }: ToastProviderProps) {
  const [toasts, setToasts] = useState<ToastData[]>([]);
  const nextId = useRef(1);

  const push = useCallback((data: Omit<ToastData, "id">) => {
    setToasts((current) => [...current, { ...data, id: nextId.current++ }]);
  }, []);

  const dismiss = useCallback((id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const value = useMemo(() => ({ push }), [push]);

  return (
    <ToastContext.Provider value={value}>
      <ToastPrimitive.Provider duration={duration} swipeDirection="right">
        {children}
        <ToastPrimitive.Viewport className="fixed bottom-0 end-0 z-50 flex w-full max-w-sm flex-col gap-2 p-4 outline-none" />
        {toasts.map((toast) => (
          <ToastPrimitive.Root
            key={toast.id}
            open
            duration={duration}
            onOpenChange={(open) => {
              if (!open) dismiss(toast.id);
            }}
            className={cn(
              "rounded-md border border-border-subtle border-s-4 bg-bg-raised p-4 shadow-lg",
              KIND_CLASSES[toast.kind],
            )}
          >
            <div className="flex items-start gap-3">
              <div className="min-w-0 flex-1">
                <ToastPrimitive.Title className="text-sm font-semibold text-text-primary">
                  {toast.title}
                </ToastPrimitive.Title>
                {toast.description ? (
                  <ToastPrimitive.Description className="mt-0.5 text-sm text-text-secondary">
                    {toast.description}
                  </ToastPrimitive.Description>
                ) : null}
              </div>
              <ToastPrimitive.Close
                aria-label="Dismiss"
                className="shrink-0 rounded-sm p-1 text-text-tertiary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary"
              >
                <X size={14} aria-hidden="true" />
              </ToastPrimitive.Close>
            </div>
            {toast.action ? (
              <ToastPrimitive.Action
                altText={toast.action.label}
                onClick={toast.action.onClick}
                className="mt-2 block text-sm font-medium text-accent hover:text-accent-hover"
              >
                {toast.action.label}
              </ToastPrimitive.Action>
            ) : null}
          </ToastPrimitive.Root>
        ))}
      </ToastPrimitive.Provider>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used within a ToastProvider");
  return useMemo(
    () => ({
      success: (data: Omit<ToastData, "id" | "kind">) => ctx.push({ ...data, kind: "success" }),
      error: (data: Omit<ToastData, "id" | "kind">) => ctx.push({ ...data, kind: "error" }),
      info: (data: Omit<ToastData, "id" | "kind">) => ctx.push({ ...data, kind: "info" }),
    }),
    [ctx],
  );
}
