import { LANGUAGE_SHORT_LABELS, SUPPORTED_LANGUAGES, useLanguage } from "@/i18n";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/cn";

/* Locale switcher (MISSION-033) — segmented control like the theme switcher. */

export function LanguageSwitcher() {
  const { language, setLanguage } = useLanguage();
  const { t } = useTranslation();

  return (
    <div
      role="group"
      className="inline-flex items-center gap-1 rounded-full border border-border-subtle bg-bg-surface p-1"
      aria-label={t("a11y.language")}
    >
      {SUPPORTED_LANGUAGES.map((code) => (
        <button
          key={code}
          type="button"
          className={cn(
            "rounded-full border-none bg-transparent px-2.5 py-1 text-sm text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary",
            language === code && "bg-accent text-bg-surface hover:bg-accent",
          )}
          aria-pressed={language === code}
          onClick={() => setLanguage(code)}
        >
          {LANGUAGE_SHORT_LABELS[code]}
        </button>
      ))}
    </div>
  );
}
