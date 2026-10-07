import { LANGUAGE_SHORT_LABELS, SUPPORTED_LANGUAGES } from "@/i18n";
import { useTranslation } from "react-i18next";
import { Segmented } from "@/components/ui";
import { usePreferences } from "@/preferences/usePreferences";

/* Locale switcher (MISSION-033) - Segmented primitive, shared with the theme
   and density switchers so every pill group in the app has one shape.

   MISSION-158 — it writes through `usePreferences`, not i18n's `setLanguage`:
   the latter only touched the boot cache, so a language picked here was
   reverted by the store on the next launch. */

export function LanguageSwitcher() {
  const { preferences, setLanguage } = usePreferences();
  const { t } = useTranslation();

  return (
    <Segmented
      aria-label={t("a11y.language")}
      value={preferences.language}
      onChange={setLanguage}
      options={SUPPORTED_LANGUAGES.map((code) => ({
        value: code,
        label: LANGUAGE_SHORT_LABELS[code],
      }))}
    />
  );
}
