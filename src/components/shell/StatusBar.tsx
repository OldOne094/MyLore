import { useTranslation } from "react-i18next";

/* DESIGN_SYSTEM.md — Status bar: persistent app state + counts at the bottom
   of the shell. Placeholder values until live data lands in M5+ (MISSION-033). */

export function StatusBar() {
  const { t } = useTranslation();

  return (
    <footer className="flex h-7 shrink-0 items-center justify-between border-t border-border-subtle bg-bg-surface px-5 text-xs text-text-tertiary">
      <span>{t("shell.status.version")}</span>
      <span className="tabular-nums">{t("shell.status.counts", { count: 0 })}</span>
    </footer>
  );
}
