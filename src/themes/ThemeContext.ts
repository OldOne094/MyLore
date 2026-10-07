import { createContext } from "react";
import type { ResolvedTheme, ThemePreference } from "./theme";

export interface ThemeContextValue {
  /** Current resolved theme ("light" | "dark") — what's actually applied. */
  theme: ResolvedTheme;
  /** User preference, including "system". */
  preference: ThemePreference;
  /** Applies a theme and mirrors the boot cache. **Does not persist**: the
      preferences store is written only by `PreferencesProvider` (MISSION-158),
      because the store wins over the boot cache at startup — a surface that
      applied its change here alone had it reverted on the next launch. An
      app-level change must go through `usePreferences().setTheme`. */
  applyPreference: (preference: ThemePreference) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);
