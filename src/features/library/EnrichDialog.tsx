import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui";
import type { EnrichView } from "@/api";
import { prettyFieldLabel, prettyValue } from "./enrichMeta";

/* MISSION-061 — Diff dialog shown after a metadata refresh: one row per changed
   field, before → after. Purely presentational; the mutation runs on the detail
   page and owns toasts/query invalidation. */

export interface EnrichDialogProps {
  view: EnrichView;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function EnrichDialog({ view, open, onOpenChange }: EnrichDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-label={t("enrich.title")} className="max-w-xl">
        <DialogTitle>{t("enrich.title")}</DialogTitle>
        <DialogDescription>{t("enrich.hint")}</DialogDescription>

        {view.changed && view.changes.length > 0 ? (
          <div className="mt-4 max-h-80 overflow-y-auto rounded-lg border border-border-subtle">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border-subtle text-left text-xs uppercase tracking-wide text-text-tertiary">
                  <th className="px-4 py-2 font-medium">{t("enrich.fieldTitle")}</th>
                  <th className="px-4 py-2 font-medium">{t("enrich.fieldBefore")}</th>
                  <th className="px-4 py-2 font-medium">{t("enrich.fieldAfter")}</th>
                </tr>
              </thead>
              <tbody>
                {view.changes.map((change) => (
                  <tr
                    key={change.field}
                    className="border-b border-border-subtle last:border-b-0 align-top"
                  >
                    <td className="px-4 py-2 text-text-secondary">
                      {prettyFieldLabel(change.field, t)}
                    </td>
                    <td className="px-4 py-2 text-text-tertiary">{prettyValue(change.before)}</td>
                    <td className="px-4 py-2 text-text-primary">{prettyValue(change.after)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="mt-4 text-sm text-text-secondary">{t("enrich.noChanges")}</p>
        )}

        <div className="mt-6 flex justify-end">
          <DialogClose asChild>
            <Button variant="secondary">{t("enrich.close")}</Button>
          </DialogClose>
        </div>
      </DialogContent>
    </Dialog>
  );
}
