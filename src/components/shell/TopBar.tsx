import { useLocation } from "react-router";
import { useTranslation } from "react-i18next";
import { NAV_ITEMS } from "@/navigation";
import { THEME_CHOICES } from "@/themes/preferences";
import { useTheme } from "@/themes/useTheme";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md — Top bar: current page title + global actions (locale,
   theme). Fully translated (MISSION-033). */

export function TopBar() {
  const location = useLocation();
  const { t } = useTranslation();
  const { preference, setPreference } = useTheme();
  const current = NAV_ITEMS.find((item) => item.path === location.pathname);

  return (
    <header className="flex h-14 shrink-0 items-center justify-between gap-3 border-b border-border-subtle px-5">
      <h1 className="truncate text-md font-semibold text-text-primary">
        {current ? t(`nav.${current.key}`) : t("shell.brand")}
      </h1>

      <div className="flex shrink-0 items-center gap-2">
        <LanguageSwitcher />
        <div className="theme-switcher" aria-label={t("theme.system")}>
          {THEME_CHOICES.map((choice) => (
            <button
              key={choice}
              type="button"
              className={cn(
                "rounded-full border-none bg-transparent px-3 py-1 text-sm text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary",
                preference === choice && "bg-accent text-bg-surface hover:bg-accent",
              )}
              aria-pressed={preference === choice}
              onClick={() => setPreference(choice)}
            >
              {t(`theme.${choice}`)}
            </button>
          ))}
        </div>
      </div>
    </header>
  );
}
