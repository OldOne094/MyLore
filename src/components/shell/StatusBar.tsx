import { useTranslation } from "react-i18next";
import { useMediaCountQuery } from "@/features/library/api";
import { TaskCenter } from "@/features/tasks/TaskCenter";

/* DESIGN_SYSTEM.md — Status bar: persistent app state + counts at the bottom
   of the shell. The title count is live (MISSION-146); it refreshes whenever
   any library mutation invalidates the `media.list` query family. The task
   centre (MISSION-155) lives here because a background task outlives the dialog
   that started it. The version comes from the build (MISSION-099) — it used to
   be a hand-edited string that drifted from package.json. */

export function StatusBar() {
  const { t } = useTranslation();
  const { data } = useMediaCountQuery();
  // Guard against a not-yet-loaded / malformed value so the plural form never
  // sees a non-number.
  const count = typeof data === "number" ? data : 0;

  return (
    <footer className="flex h-7 shrink-0 items-center justify-between border-t border-border-subtle bg-bg-surface px-5 text-xs text-text-tertiary">
      <span className="flex items-center gap-3">
        <span>v{__APP_VERSION__}</span>
        <TaskCenter />
      </span>
      <span className="tabular-nums">{t("shell.status.counts", { count })}</span>
    </footer>
  );
}
