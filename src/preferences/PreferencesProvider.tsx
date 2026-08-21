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
   boot cache so the pre-paint boot and this store never diverge. */

export function PreferencesProvider({ children }: { children: ReactNode }) {
  const { setPreference } = useTheme();
  const [preferences, setPreferences] = useState<Preferences>(() => ({
    theme: readPreference(),
    language: readLanguage(),
    density: "comfortable",
  }));
  const repositoryRef = useRef(getPreferencesRepository());

  useEffect(() => {
    let cancelled = false;
    void repositoryRef.current
      .load()
      .then((stored) => {
        if (cancelled || stored === null) return;
        setPreferences(stored);
        setPreference(stored.theme);
        void setLanguage(stored.language);
      })
      .catch(() => {
        /* Store unavailable — keep boot values. */
      });
    return () => {
      cancelled = true;
    };
  }, [setPreference]);

  const setTheme = useCallback(
    (theme: Preferences["theme"]) => {
      setPreferences((current) => {
        const next = { ...current, theme };
        void repositoryRef.current.save(next);
        setPreference(theme);
        return next;
      });
    },
    [setPreference],
  );

  const setLocale = useCallback((language: Preferences["language"]) => {
    setPreferences((current) => {
      const next = { ...current, language };
      void repositoryRef.current.save(next);
      void setLanguage(language);
      return next;
    });
  }, []);

  const setDensity = useCallback((density: Preferences["density"]) => {
    setPreferences((current) => {
      const next = { ...current, density };
      void repositoryRef.current.save(next);
      return next;
    });
  }, []);

  // Reflect the density tier on the root so the CSS variable overrides in
  // tokens.css apply everywhere (MISSION-095).
  useEffect(() => {
    document.documentElement.dataset.density = preferences.density;
  }, [preferences.density]);

  const value = useMemo(
    () => ({ preferences, setTheme, setLanguage: setLocale, setDensity }),
    [preferences, setTheme, setLocale, setDensity],
  );

  return <PreferencesContext.Provider value={value}>{children}</PreferencesContext.Provider>;
}
