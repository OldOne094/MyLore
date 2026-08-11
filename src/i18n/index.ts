import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { useTranslation } from "react-i18next";
export type { AppLanguage } from "./locales";
import { resources, type AppLanguage } from "./locales";

/* MISSION-033 — i18n bootstrap. Language persists in localStorage
   ("mylore.lang"), defaults to "system"/browser preference, and toggling applies
   `lang` + `dir` to <html> so RTL mirrors automatically (logical layout). */

export const LOCALE_STORAGE_KEY = "mylore.lang";
export const SUPPORTED_LANGUAGES: AppLanguage[] = ["en", "ar"];

export function isAppLanguage(value: unknown): value is AppLanguage {
  return SUPPORTED_LANGUAGES.includes(value as AppLanguage);
}

export function browserLanguage(): AppLanguage {
  const candidates = [navigator.language, ...(navigator.languages ?? [])];
  const primary = candidates.map((code) => code.toLowerCase());
  if (primary.some((code) => code.startsWith("ar"))) return "ar";
  return "en";
}

export function readLanguage(): AppLanguage {
  try {
    const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
    return isAppLanguage(stored) ? stored : browserLanguage();
  } catch {
    return browserLanguage();
  }
}

export function isRtl(language: AppLanguage): boolean {
  return language === "ar";
}

/** Applies lang + dir to <html> and returns the effective language. */
export function applyLanguage(language: AppLanguage): AppLanguage {
  const root = document.documentElement;
  root.setAttribute("lang", language);
  root.setAttribute("dir", isRtl(language) ? "rtl" : "ltr");
  return language;
}

void i18n.use(initReactI18next).init({
  resources,
  lng: applyLanguage(readLanguage()),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
  react: { useSuspense: false },
  returnEmptyString: false,
});

/** Sets a language persistently, applies RTL/LTR, and re-renders via i18next. */
export function setLanguage(language: AppLanguage): Promise<unknown> {
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, language);
  } catch {
    /* Storage unavailable (private mode) — the session language still applies. */
  }
  applyLanguage(language);
  return i18n.changeLanguage(language);
}

/** Bootstraps the locale before first paint (mirrors initTheme). */
export function initI18n(): AppLanguage {
  const language = (i18n.resolvedLanguage ?? readLanguage()) as AppLanguage;
  return applyLanguage(language);
}

/** Read/switch the app language with RTL/LTR applied automatically. */
export function useLanguage() {
  const { i18n: instance } = useTranslation();
  const language = (instance.resolvedLanguage ?? "en") as AppLanguage;
  return { language, setLanguage: (next: AppLanguage) => setLanguage(next) };
}

export default i18n;
