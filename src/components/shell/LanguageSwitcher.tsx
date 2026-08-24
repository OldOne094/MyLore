import { LANGUAGE_SHORT_LABELS, SUPPORTED_LANGUAGES, useLanguage } from "@/i18n";
import { useTranslation } from "react-i18next";
import { Segmented } from "@/components/ui";

/* Locale switcher (MISSION-033) - Segmented primitive, shared with the theme
   and density switchers so every pill group in the app has one shape. */

export function LanguageSwitcher() {
  const { language, setLanguage } = useLanguage();
  const { t } = useTranslation();

  return (
    <Segmented
      aria-label={t("a11y.language")}
      value={language}
      onChange={setLanguage}
      options={SUPPORTED_LANGUAGES.map((code) => ({
        value: code,
        label: LANGUAGE_SHORT_LABELS[code],
      }))}
    />
  );
}
