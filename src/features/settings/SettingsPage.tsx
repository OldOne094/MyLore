import { useTranslation } from "react-i18next";
import { THEME_CHOICES } from "@/themes/preferences";
import { LANGUAGE_SHORT_LABELS, SUPPORTED_LANGUAGES } from "@/i18n";
import { usePreferences } from "@/preferences/usePreferences";
import { cn } from "@/lib/cn";
import { ProvidersSection } from "./ProvidersSection";
import { ExportSection } from "./ExportSection";
import { BackupsSection } from "./BackupsSection";

/* Settings page (MISSION-034) — persistent theme + language preferences. These
   mirror the TopBar switchers but manage the full Preferences store. */

interface SectionProps {
  title: string;
  hint: string;
  children: React.ReactNode;
}

function Section({ title, hint, children }: SectionProps) {
  return (
    <section className="rounded-md border border-border-subtle bg-bg-surface p-6">
      <h2 className="text-sm font-semibold text-text-primary">{title}</h2>
      <p className="mt-1 text-sm text-text-secondary">{hint}</p>
      <div className="mt-4">{children}</div>
    </section>
  );
}

export function SettingsPage() {
  const { t } = useTranslation();
  const { preferences, setTheme, setLanguage } = usePreferences();

  return (
    <div className="mx-auto flex w-full max-w-xl flex-col gap-6 p-6">
      <Section title={t("settings.theme")} hint={t("settings.themeHint")}>
        <div
          role="group"
          aria-label={t("settings.theme")}
          className="inline-flex items-center gap-1 rounded-full border border-border-subtle bg-bg-raised p-1"
        >
          {THEME_CHOICES.map((choice) => (
            <button
              key={choice}
              type="button"
              className={cn(
                "rounded-full border-none bg-transparent px-3 py-1 text-sm text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary",
                preferences.theme === choice && "bg-accent text-bg-surface hover:bg-accent",
              )}
              aria-pressed={preferences.theme === choice}
              onClick={() => setTheme(choice)}
            >
              {t(`theme.${choice}`)}
            </button>
          ))}
        </div>
      </Section>

      <Section title={t("settings.language")} hint={t("settings.languageHint")}>
        <div
          role="group"
          aria-label={t("settings.language")}
          className="inline-flex items-center gap-1 rounded-full border border-border-subtle bg-bg-raised p-1"
        >
          {SUPPORTED_LANGUAGES.map((code) => (
            <button
              key={code}
              type="button"
              className={cn(
                "rounded-full border-none bg-transparent px-3 py-1 text-sm text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary",
                preferences.language === code && "bg-accent text-bg-surface hover:bg-accent",
              )}
              aria-pressed={preferences.language === code}
              onClick={() => setLanguage(code)}
            >
              {LANGUAGE_SHORT_LABELS[code]}
            </button>
          ))}
        </div>
      </Section>

      <Section title={t("settings.providers")} hint={t("settings.providersHint")}>
        <ProvidersSection />
      </Section>

      <ExportSection />

      <BackupsSection />
    </div>
  );
}
