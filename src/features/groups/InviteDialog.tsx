/* MISSION-117 — Out-of-band invite.

   Presentational: the group page creates the invite when the button is pressed,
   so the dialog has no effect-driven state of its own. The link carries the
   group key, so the copy says to send it privately rather than implying the
   relay keeps it secret. */

import { useTranslation } from "react-i18next";
import { Button, Dialog, DialogContent, DialogTitle, Skeleton } from "@/components/ui";
import type { GroupInviteView } from "@/api";
import { CopyButton } from "@/features/groups/CopyButton";

export interface InviteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  invite: GroupInviteView | null;
  error: string | null;
}

export function InviteDialog({ open, onOpenChange, invite, error }: InviteDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogTitle>{t("groupsPage.inviteTitle")}</DialogTitle>
        <p className="mt-2 text-sm text-text-secondary">{t("groupsPage.inviteHint")}</p>

        {error ? (
          <p className="mt-4 text-sm text-danger">{error}</p>
        ) : invite ? (
          <>
            <p className="mt-4 rounded-sm bg-bg-hover px-2.5 py-2 font-mono text-xs break-all text-text-secondary">
              {invite.link}
            </p>
            {invite.relays.length === 0 ? (
              <p className="mt-3 text-sm text-warn">{t("groupsPage.inviteNoRelays")}</p>
            ) : null}
            <div className="mt-4 flex justify-end gap-2">
              <CopyButton value={invite.link} label={t("groupsPage.copyLink")} />
              <Button variant="secondary" onClick={() => onOpenChange(false)}>
                {t("groupsPage.done")}
              </Button>
            </div>
          </>
        ) : (
          <div role="status" aria-label={t("groupsPage.inviteTitle")} className="mt-4">
            <Skeleton className="h-9 w-full" />
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
