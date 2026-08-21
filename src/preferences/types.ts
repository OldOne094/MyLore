import type { ThemePreference } from "@/themes/theme";
import type { AppLanguage } from "@/i18n";

/* MISSION-034 — Persisted preferences model. The source of truth is a Tauri
   settings store at runtime; localStorage backends it in non-Tauri contexts
   and as a boot-time cache so first paint never blocks on IPC. */

/** Global UI density (MISSION-095): comfortable is the default spacing scale,
    compact shrinks control heights for denser screens. */
export type UiDensity = "comfortable" | "compact";

export function isUiDensity(value: unknown): value is UiDensity {
  return value === "comfortable" || value === "compact";
}

export interface Preferences {
  theme: ThemePreference;
  language: AppLanguage;
  density: UiDensity;
}

export const DEFAULT_PREFERENCES: Preferences = {
  theme: "system",
  language: "en",
  density: "comfortable",
};
