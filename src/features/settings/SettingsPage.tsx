import { useTranslation } from "react-i18next";
import { THEME_CHOICES } from "@/themes/preferences";
import { LANGUAGE_SHORT_LABELS, SUPPORTED_LANGUAGES } from "@/i18n";
import { usePreferences } from "@/preferences/usePreferences";
import { ACCENT_CHOICES, ACCENT_SWATCH } from "@/preferences/types";
import { Segmented } from "@/components/ui";
import { cn } from "@/lib/cn";
import { ProvidersSection } from "./ProvidersSection";
import { ExportSection } from "./ExportSection";
import { BackupsSection } from "./BackupsSection";
import { SecuritySection } from "./SecuritySection";
import { ProfileSection } from "./ProfileSection";

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
  const { preferences, setTheme, setLanguage, setDensity, setAccent } = usePreferences();

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
        <div className="mt-4 flex flex-col gap-2">
          <p className="text-sm font-medium text-text-secondary">{t("settings.accent")}</p>
          <div className="flex items-center gap-2">
            {ACCENT_CHOICES.map((choice) => (
              <button
                key={choice}
                type="button"
                aria-label={t(`settings.accent_${choice}`)}
                aria-pressed={preferences.accent === choice}
                title={t(`settings.accent_${choice}`)}
                onClick={() => setAccent(choice)}
                style={{ backgroundColor: ACCENT_SWATCH[choice] }}
                className={cn(
                  "size-8 rounded-full border transition-transform duration-150 ease-out hover:scale-110",
                  preferences.accent === choice
                    ? "border-transparent ring-2 ring-accent ring-offset-2 ring-offset-bg-surface"
                    : "border-border-strong",
                )}
              />
            ))}
          </div>
        </div>
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

      <Section title={t("settings.profile")} hint={t("settings.profileHint")}>
        <ProfileSection />
      </Section>

      <Section title={t("settings.providers")} hint={t("settings.providersHint")}>
        <ProvidersSection />
      </Section>

      <Section title={t("settings.security")} hint={t("settings.securityHint")}>
        <SecuritySection />
      </Section>

      <ExportSection />

      <BackupsSection />
    </div>
  );
}
