/* Theme resolution + application (MISSION-030).
   System preference is the default; an explicit light/dark override persists.
   Framework-free so it can run before React mounts (no FOUC). */

export type ThemePreference = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

export const THEME_STORAGE_KEY = "mylore.theme";
export const THEME_ATTRIBUTE = "data-theme";
export const THEME_QUERY = "(prefers-color-scheme: dark)";

export function isThemePreference(value: unknown): value is ThemePreference {
  return value === "light" || value === "dark" || value === "system";
}

export function matchSystemTheme(): ResolvedTheme {
  return typeof window.matchMedia === "function" && window.matchMedia(THEME_QUERY).matches
    ? "dark"
    : "light";
}

export function resolveTheme(preference: ThemePreference): ResolvedTheme {
  return preference === "system" ? matchSystemTheme() : preference;
}

/** Reads the persisted override; falls back to "system". */
export function readPreference(storage: Pick<Storage, "getItem"> = localStorage): ThemePreference {
  try {
    const value = storage.getItem(THEME_STORAGE_KEY);
    return isThemePreference(value) ? value : "system";
  } catch {
    return "system";
  }
}

export function writePreference(
  preference: ThemePreference,
  storage: Pick<Storage, "setItem"> = localStorage,
): void {
  try {
    storage.setItem(THEME_STORAGE_KEY, preference);
  } catch {
    /* Storage can be unavailable (private mode); the in-memory value still applies. */
  }
}

/** Sets `data-theme` on <html> and returns the resolved theme. */
export function applyTheme(preference: ThemePreference): ResolvedTheme {
  const resolved = resolveTheme(preference);
  document.documentElement.setAttribute(THEME_ATTRIBUTE, resolved);
  return resolved;
}

export interface ThemeSystem {
  preference: ThemePreference;
  resolved: ResolvedTheme;
  /** Applies and persists a preference, then returns the resolved theme. */
  setPreference(preference: ThemePreference): ResolvedTheme;
  /** Re-resolves from the current system preference (used on media-query change). */
  sync(): ResolvedTheme;
}

/** Creates a theme controller bound to the document and an optional storage. */
export function createThemeSystem(
  storage: Pick<Storage, "getItem" | "setItem"> = localStorage,
): ThemeSystem {
  const preference = readPreference(storage);
  const resolved = applyTheme(preference);

  const setPreference = (next: ThemePreference): ResolvedTheme => {
    writePreference(next, storage);
    return applyTheme(next);
  };

  const sync = (): ResolvedTheme => applyTheme(preference);

  return { preference, resolved, setPreference, sync };
}

/** Applies the persisted (or system) preference now; used before render. */
export function initTheme(
  storage: Pick<Storage, "getItem" | "setItem"> = localStorage,
): ResolvedTheme {
  return createThemeSystem(storage).resolved;
}
