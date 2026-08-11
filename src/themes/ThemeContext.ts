import { createContext } from "react";
import type { ResolvedTheme, ThemePreference } from "./theme";

export interface ThemeContextValue {
  /** Current resolved theme ("light" | "dark") — what's actually applied. */
  theme: ResolvedTheme;
  /** User preference, including "system". */
  preference: ThemePreference;
  setPreference: (preference: ThemePreference) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);
