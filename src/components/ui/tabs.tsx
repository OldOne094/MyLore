import {
  createContext,
  useContext,
  useId,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Tabs: detail page sections. Framework-free tablist with
   full keyboard nav (←/→ move, Home/End, Enter/Space activates) and
   `aria-selected`/`role=tabpanel` semantics. */

interface TabsContextValue {
  value: string;
  onSelect: (value: string) => void;
  baseId: string;
}

const TabsContext = createContext<TabsContextValue | null>(null);

function useTabsContext() {
  const ctx = useContext(TabsContext);
  if (!ctx) throw new Error("Tabs components must be used inside <Tabs>");
  return ctx;
}

export interface TabsProps {
  value: string;
  onValueChange: (value: string) => void;
  children: ReactNode;
  className?: string;
}

export function Tabs({ value, onValueChange, children, className }: TabsProps) {
  const baseId = useId();
  return (
    <TabsContext.Provider value={{ value, onSelect: onValueChange, baseId }}>
      <div className={cn("flex flex-col", className)}>{children}</div>
    </TabsContext.Provider>
  );
}

export interface TabsListProps {
  children: ReactNode;
  ariaLabel?: string;
  className?: string;
}

export function TabsList({ children, ariaLabel, className }: TabsListProps) {
  const { baseId } = useTabsContext();
  const [focusedIndex, setFocusedIndex] = useState(0);

  const moveFocus = (list: HTMLElement, from: number, delta: number) => {
    const buttons = Array.from(list.querySelectorAll<HTMLButtonElement>("[role='tab']"));
    const next = (from + delta + buttons.length) % buttons.length;
    buttons[next]?.focus();
    setFocusedIndex(next);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const list = event.currentTarget;
    switch (event.key) {
      case "ArrowRight":
        event.preventDefault();
        moveFocus(list, focusedIndex, 1);
        break;
      case "ArrowLeft":
        event.preventDefault();
        moveFocus(list, focusedIndex, -1);
        break;
      case "Home":
        event.preventDefault();
        list.querySelector<HTMLButtonElement>("[role='tab']")?.focus();
        setFocusedIndex(0);
        break;
      case "End": {
        event.preventDefault();
        const buttons = list.querySelectorAll<HTMLButtonElement>("[role='tab']");
        buttons[buttons.length - 1]?.focus();
        setFocusedIndex(buttons.length - 1);
        break;
      }
    }
  };

  return (
    <div
      id={baseId}
      role="tablist"
      aria-label={ariaLabel}
      onKeyDown={onKeyDown}
      className={cn("flex flex-wrap gap-1 border-b border-border-subtle", className)}
    >
      {children}
    </div>
  );
}

export interface TabsTriggerProps {
  value: string;
  children: ReactNode;
}

export function TabsTrigger({ value, children }: TabsTriggerProps) {
  const { value: current, onSelect, baseId } = useTabsContext();
  const active = current === value;
  return (
    <button
      type="button"
      role="tab"
      id={`${baseId}-tab-${value}`}
      aria-selected={active}
      aria-controls={`${baseId}-panel-${value}`}
      tabIndex={active ? 0 : -1}
      onClick={() => onSelect(value)}
      className={cn(
        "-mb-px border-b-2 px-1 py-2 text-sm font-medium transition-colors duration-150 ease-out",
        active
          ? "border-accent text-accent"
          : "border-transparent text-text-secondary hover:border-border-strong hover:text-text-primary",
      )}
    >
      {children}
    </button>
  );
}

export interface TabsContentProps {
  value: string;
  children: ReactNode;
  className?: string;
}

export function TabsContent({ value, children, className }: TabsContentProps) {
  const { value: current, baseId } = useTabsContext();
  if (current !== value) return null;
  return (
    <div
      role="tabpanel"
      id={`${baseId}-panel-${value}`}
      aria-labelledby={`${baseId}-tab-${value}`}
      className={cn("flex flex-col", className)}
    >
      {children}
    </div>
  );
}
