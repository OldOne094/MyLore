import { useTranslation } from "react-i18next";
import { useMediaCountQuery } from "@/features/library/api";

/* DESIGN_SYSTEM.md — Status bar: persistent app state + counts at the bottom
   of the shell. The title count is live (MISSION-146); it refreshes whenever
   any library mutation invalidates the `media.list` query family. */

export function StatusBar() {
  const { t } = useTranslation();
  const { data } = useMediaCountQuery();
  // Guard against a not-yet-loaded / malformed value so the plural form never
  // sees a non-number.
  const count = typeof data === "number" ? data : 0;

  return (
    <footer className="flex h-7 shrink-0 items-center justify-between border-t border-border-subtle bg-bg-surface px-5 text-xs text-text-tertiary">
      <span>{t("shell.status.version")}</span>
      <span className="tabular-nums">{t("shell.status.counts", { count })}</span>
    </footer>
  );
}
