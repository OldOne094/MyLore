import { Store } from "@tauri-apps/plugin-store";
import { isAppLanguage } from "@/i18n";
import { isThemePreference } from "@/themes/theme";
import { DEFAULT_PREFERENCES, isUiDensity, type Preferences } from "./types";

/* MISSION-034 — Preference persistence. Runtime backend: tauri-plugin-store
   (settings.json) in the desktop shell; fallback backend: localStorage so the
   app works in browsers/tests. The boot-cache keys (mylore.theme / mylore.lang)
   stay mirrored on every write so initTheme/initI18n and this store agree. */

export const PREFERENCES_KEY = "mylore.preferences";
export const SETTINGS_STORE_FILE = "settings.json";

export interface PreferencesRepository {
  /** Resolves stored preferences, or null when nothing usable is stored. */
  load(): Promise<Preferences | null>;
  save(preferences: Preferences): Promise<void>;
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Coerces parsed storage into a valid Preferences; null when the shape is unusable. */
export function parsePreferences(value: unknown): Preferences | null {
  if (value === null || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  return {
    theme: isThemePreference(record.theme) ? record.theme : DEFAULT_PREFERENCES.theme,
    language: isAppLanguage(record.language) ? record.language : DEFAULT_PREFERENCES.language,
    density: isUiDensity(record.density) ? record.density : DEFAULT_PREFERENCES.density,
  };
}

class LocalPreferencesRepository implements PreferencesRepository {
  async load(): Promise<Preferences | null> {
    try {
      const raw = localStorage.getItem(PREFERENCES_KEY);
      return raw === null ? null : parsePreferences(JSON.parse(raw));
    } catch {
      return null;
    }
  }

  async save(preferences: Preferences): Promise<void> {
    try {
      localStorage.setItem(PREFERENCES_KEY, JSON.stringify(preferences));
    } catch {
      /* Storage unavailable (private mode) — preferences stay session-only. */
    }
  }
}

class TauriPreferencesRepository implements PreferencesRepository {
  private storePromise?: Promise<Store>;

  private store(): Promise<Store> {
    this.storePromise ??= Store.load(SETTINGS_STORE_FILE);
    return this.storePromise;
  }

  async load(): Promise<Preferences | null> {
    const store = await this.store();
    const value = await store.get<unknown>(PREFERENCES_KEY);
    return value === undefined || value === null ? null : parsePreferences(value);
  }

  async save(preferences: Preferences): Promise<void> {
    const store = await this.store();
    await store.set(PREFERENCES_KEY, preferences);
    await store.save();
  }
}

let cached: PreferencesRepository | null = null;

/** Returns the repository for the current runtime (Tauri store or localStorage). */
export function getPreferencesRepository(): PreferencesRepository {
  cached ??= isTauriRuntime() ? new TauriPreferencesRepository() : new LocalPreferencesRepository();
  return cached;
}
