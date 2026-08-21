import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui";
import { useShortcuts } from "@/shortcuts/useShortcuts";
import { formatKeyCombo } from "@/shortcuts/keys";
import { OPEN_SHORTCUTS_EVENT, SHORTCUTS } from "@/shortcuts/map";

/* MISSION-090 — Shortcuts help dialog: the complete keyboard map, opened
   with "?" or the palette's "Keyboard shortcuts" command. The list renders
   from the shared SHORTCUTS map so it can never drift from the bindings. */

export function ShortcutsDialog() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  useShortcuts([{ combo: "?", handler: () => setOpen(true) }]);

  useEffect(() => {
    const handler = () => setOpen(true);
    window.addEventListener(OPEN_SHORTCUTS_EVENT, handler);
    return () => window.removeEventListener(OPEN_SHORTCUTS_EVENT, handler);
  }, []);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent closeLabel={t("a11y.close")} className="max-w-md">
        <DialogTitle>{t("shortcuts.title")}</DialogTitle>
        <DialogDescription>{t("shortcuts.hint")}</DialogDescription>
        <ul className="mt-4 flex flex-col divide-y divide-border-subtle">
          {SHORTCUTS.map((shortcut) => (
            <li
              key={shortcut.id}
              className="flex items-center justify-between gap-3 py-2.5 text-sm"
            >
              <span className="text-text-primary">{t(`shortcuts.${shortcut.labelKey}`)}</span>{" "}
              <kbd className="shrink-0 rounded-sm border border-border-subtle bg-bg-base px-1.5 py-0.5 text-xs tabular-nums text-text-tertiary">
                {formatKeyCombo(shortcut.combo)}
              </kbd>
            </li>
          ))}
        </ul>
      </DialogContent>
    </Dialog>
  );
}
