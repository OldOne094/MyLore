import { createContext } from "react";
import type { AppLanguage } from "@/i18n";
import type { ThemePreference } from "@/themes/theme";
import type { Preferences } from "./types";

/* MISSION-034 — Context contract for reading and updating app preferences. */

export interface PreferencesContextValue {
  preferences: Preferences;
  setTheme: (preference: ThemePreference) => void;
  setLanguage: (language: AppLanguage) => void;
}

export const PreferencesContext = createContext<PreferencesContextValue | null>(null);
