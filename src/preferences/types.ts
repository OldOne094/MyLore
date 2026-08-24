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

/** Accent color personalization (user freedom layer). "classic" is the
    built-in terracotta; anything else swaps the accent triad via the
    data-accent attribute in tokens.css. */
export type AccentChoice = "classic" | "ocean" | "violet" | "emerald" | "rose" | "amber";

export const ACCENT_CHOICES: AccentChoice[] = [
  "classic",
  "ocean",
  "violet",
  "emerald",
  "rose",
  "amber",
];

export function isAccentChoice(value: unknown): value is AccentChoice {
  return ACCENT_CHOICES.includes(value as AccentChoice);
}

/** Swatch preview colors (light-theme values) for the settings picker. */
export const ACCENT_SWATCH: Record<AccentChoice, string> = {
  classic: "#b4541f",
  ocean: "#2563eb",
  violet: "#7c3aed",
  emerald: "#047857",
  rose: "#be123c",
  amber: "#b45309",
};

export interface Preferences {
  theme: ThemePreference;
  language: AppLanguage;
  density: UiDensity;
  accent: AccentChoice;
}

export const DEFAULT_PREFERENCES: Preferences = {
  theme: "system",
  language: "en",
  density: "comfortable",
  accent: "classic",
};
