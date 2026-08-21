import { useEffect, useState } from "react";

/* MISSION-094 — Debounce primitive for type-ahead surfaces: the returned
   value lags the input by `delayMs` so rapid keystrokes collapse into one
   query. Resets the timer on every change. */

export function useDebouncedValue<T>(value: T, delayMs = 200): T {
  const [debounced, setDebounced] = useState(value);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(timer);
  }, [value, delayMs]);

  return debounced;
}
