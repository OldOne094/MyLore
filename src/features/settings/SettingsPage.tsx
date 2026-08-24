import { useTranslation } from "react-i18next";
import { THEME_CHOICES } from "@/themes/preferences";
import { LANGUAGE_SHORT_LABELS, SUPPORTED_LANGUAGES } from "@/i18n";
import { usePreferences } from "@/preferences/usePreferences";
import { Segmented } from "@/components/ui";
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
  const { preferences, setTheme, setLanguage, setDensity } = usePreferences();

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6 p-6">
      <Section title={t("settings.theme")} hint={t("settings.themeHint")}>
        <Segmented
          aria-label={t("settings.theme")}
          value={preferences.theme}
          onChange={setTheme}
          options={THEME_CHOICES.map((choice) => ({
            value: choice,
            label: t(`theme.${choice}`),
          }))}
        />
      </Section>

      <Section title={t("settings.language")} hint={t("settings.languageHint")}>
        <Segmented
          aria-label={t("settings.language")}
          value={preferences.language}
          onChange={setLanguage}
          options={SUPPORTED_LANGUAGES.map((code) => ({
            value: code,
            label: LANGUAGE_SHORT_LABELS[code],
          }))}
        />
      </Section>

      <Section title={t("settings.density")} hint={t("settings.densityHint")}>
        <Segmented
          aria-label={t("settings.density")}
          value={preferences.density}
          onChange={setDensity}
          options={[
            { value: "comfortable", label: t("settings.density_comfortable") },
            { value: "compact", label: t("settings.density_compact") },
          ]}
        />
      </Section>

      <Section title={t("settings.providers")} hint={t("settings.providersHint")}>
        <ProvidersSection />
      </Section>

      <ExportSection />

      <BackupsSection />
    </div>
  );
}
