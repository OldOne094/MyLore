import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { ThemeContext, type ThemeContextValue } from "./ThemeContext";
import {
  applyTheme,
  isThemePreference,
  readPreference,
  resolveTheme,
  THEME_QUERY,
  writePreference,
  type ThemePreference,
} from "./theme";

function storedPreference(): ThemePreference {
  const value = readPreference();
  return isThemePreference(value) ? value : "system";
}

/** Applies the theme to <html> and keeps it in sync with the system setting. */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [preference, setPreferenceState] = useState<ThemePreference>(storedPreference);
  const [, setSystemTick] = useState(0);

  // `theme` derives directly from preference, so it is always consistent with
  // what applyTheme writes — no state-in-effect churn.
  const theme = resolveTheme(preference);

  useEffect(() => {
    applyTheme(preference);
    if (preference !== "system" || typeof window.matchMedia !== "function") return;
    const media = window.matchMedia(THEME_QUERY);
    const onChange = () => {
      applyTheme("system");
      setSystemTick((tick) => tick + 1);
    };
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [preference]);

  const setPreference = useCallback((next: ThemePreference) => {
    writePreference(next);
    setPreferenceState(next);
  }, []);

  const value = useMemo<ThemeContextValue>(
    () => ({ theme, preference, setPreference }),
    [theme, preference, setPreference],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
