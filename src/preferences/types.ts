import type { ThemePreference } from "@/themes/theme";
import type { AppLanguage } from "@/i18n";

/* MISSION-034 — Persisted preferences model. The source of truth is a Tauri
   settings store at runtime; localStorage backends it in non-Tauri contexts
   and as a boot-time cache so first paint never blocks on IPC. */

export interface Preferences {
  theme: ThemePreference;
  language: AppLanguage;
}

export const DEFAULT_PREFERENCES: Preferences = {
  theme: "system",
  language: "en",
};
