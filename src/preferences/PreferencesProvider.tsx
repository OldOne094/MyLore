import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { readLanguage, setLanguage } from "@/i18n";
import { readPreference } from "@/themes/theme";
import { useTheme } from "@/themes/useTheme";
import { PreferencesContext } from "./PreferencesContext";
import { getPreferencesRepository } from "./repository";
import type { Preferences } from "./types";

/* MISSION-034 — The single context for reading and updating app preferences.
   Renders immediately with boot values (no flash), then reconciles with the
   persisted store once loaded. Every change is persisted and mirrored to the
   boot cache so the pre-paint boot and this store never diverge.

   MISSION-158 — this provider is the **only writer** of preferences, and it has
   to be: the store beats the boot cache at startup, so a surface that applied
   its change through the theme/i18n singletons alone (the TopBar theme switcher
   and the language switcher both did) had it reverted on the next launch as
   soon as `settings.json` held a snapshot. Every mutation now goes through
   `persist`, which writes the store and the caches together. */

export function PreferencesProvider({ children }: { children: ReactNode }) {
  const { applyPreference } = useTheme();
  const [preferences, setPreferences] = useState<Preferences>(() => ({
    theme: readPreference(),
    language: readLanguage(),
    density: "comfortable",
    accent: "classic",
  }));
  const repositoryRef = useRef(getPreferencesRepository());
  /* Armed by the first user mutation: a choice made while the store is still
     loading must not be reverted by the snapshot that lands after it. */
  const mutatedRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    void repositoryRef.current
      .load()
      .then((stored) => {
        if (cancelled || stored === null || mutatedRef.current) return;
        setPreferences(stored);
        applyPreference(stored.theme);
        void setLanguage(stored.language);
      })
      .catch(() => {
        /* Store unavailable — keep boot values. */
      });
    return () => {
      cancelled = true;
    };
  }, [applyPreference]);

  /** The one write path: store + boot cache + in-memory state together. */
  const persist = useCallback((next: Preferences) => {
    mutatedRef.current = true;
    setPreferences(next);
    void repositoryRef.current.save(next);
  }, []);

  const setTheme = useCallback(
    (theme: Preferences["theme"]) => {
      persist({ ...preferences, theme });
      applyPreference(theme);
    },
    [applyPreference, persist, preferences],
  );

  const setLocale = useCallback(
    (language: Preferences["language"]) => {
      persist({ ...preferences, language });
      void setLanguage(language);
    },
    [persist, preferences],
  );

  const setDensity = useCallback(
    (density: Preferences["density"]) => persist({ ...preferences, density }),
    [persist, preferences],
  );

  const setAccent = useCallback(
    (accent: Preferences["accent"]) => persist({ ...preferences, accent }),
    [persist, preferences],
  );

  // Reflect the accent choice on the root so the CSS variable overrides in
  // tokens.css apply everywhere (MISSION-112 freedom layer).
  useEffect(() => {
    if (preferences.accent === "classic") {
      delete document.documentElement.dataset.accent;
    } else {
      document.documentElement.dataset.accent = preferences.accent;
    }
  }, [preferences.accent]);

  // Reflect the density tier on the root so the CSS variable overrides in
  // tokens.css apply everywhere (MISSION-095).
  useEffect(() => {
    document.documentElement.dataset.density = preferences.density;
  }, [preferences.density]);

  const value = useMemo(
    () => ({ preferences, setTheme, setLanguage: setLocale, setDensity, setAccent }),
    [preferences, setTheme, setLocale, setDensity, setAccent],
  );

  return <PreferencesContext.Provider value={value}>{children}</PreferencesContext.Provider>;
}
