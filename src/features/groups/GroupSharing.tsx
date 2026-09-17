/* MISSION-117 — How a group leaves this device.

   One panel for everything transport-shaped: the shared key's state, the relay
   set (owner-only, per ARCHITECTURE §6), and the manual `group_state.json` file —
   the seam that carries shelves and members, which the relay sync does not. The
   panel states which is which, because assuming the sync moves shelves would be
   wrong. */

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, TextareaField, useToast } from "@/components/ui";
import {
  useExportGroupFile,
  useGroupKeyStatusQuery,
  useGroupRelaysQuery,
  useImportGroupFile,
  useSetGroupRelays,
} from "@/features/groups/api";

export interface GroupSharingProps {
  groupId: string;
  groupName: string;
  p2p: boolean;
  isOwner: boolean;
}

export function GroupSharing({ groupId, groupName, p2p, isOwner }: GroupSharingProps) {
  const { t } = useTranslation();
  const toast = useToast();

  const keyStatus = useGroupKeyStatusQuery(groupId);
  const relaysQuery = useGroupRelaysQuery(groupId, p2p);
  const setRelays = useSetGroupRelays(groupId);
  const exportFile = useExportGroupFile();
  const importFile = useImportGroupFile();

  const [draft, setDraft] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  const relays = relaysQuery.data?.relays ?? [];
  const pending = relaysQuery.data?.pending ?? 0;
  const text = draft ?? relays.join("\n");

  const onExport = () => {
    exportFile.mutate(
      { groupId, fileName: `${groupName}.group.json` },
      {
        onSuccess: (report) => {
          if (!report) return;
          toast.success({
            title: t("groupsPage.exportToast", {
              groups: report.groups,
              members: report.members,
              shelf: report.shelf,
              notes: report.notes,
            }),
          });
        },
        onError: () => toast.error({ title: t("groupsPage.exportErrorToast") }),
      },
    );
  };

  const onFilePicked = (file: File | undefined) => {
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      importFile.mutate(String(reader.result ?? ""), {
        onSuccess: (report) =>
          toast.success({
            title: t("groupsPage.importToast", {
              groups: report.groups,
              shelf: report.shelf,
              notes: report.notes,
              skipped: report.skipped,
            }),
          }),
        onError: () => toast.error({ title: t("groupsPage.importErrorToast") }),
      });
    };
    reader.readAsText(file);
  };

  return (
    <section
      aria-label={t("groupsPage.sharingHeading")}
      className="rounded-md border border-border-subtle bg-bg-surface p-4"
    >
      <h2 className="text-sm font-semibold text-text-primary">{t("groupsPage.sharingHeading")}</h2>

      {p2p ? (
        <p className="mt-2 text-sm text-text-secondary">
          {keyStatus.data?.has_key
            ? t("groupsPage.keyPresent", { id: keyStatus.data.key_id ?? "" })
            : t("groupsPage.keyAbsent")}
        </p>
      ) : (
        <p className="mt-2 text-sm text-text-tertiary">{t("groupsPage.p2pUnavailable")}</p>
      )}

      {p2p ? (
        <div className="mt-4">
          <TextareaField
            label={t("groupsPage.relaysLabel")}
            value={text}
            rows={3}
            placeholder="wss://relay.example.com"
            disabled={!isOwner}
            onChange={(event) => setDraft(event.target.value)}
          />
          <p className="mt-2 text-sm text-text-tertiary">
            {isOwner ? t("groupsPage.relaysHint") : t("groupsPage.relaysOwnerOnly")}
          </p>
          {pending > 0 ? (
            <p className="mt-1 text-sm text-text-secondary">
              {t("groupsPage.pendingCount", { count: pending })}
            </p>
          ) : null}
          {isOwner ? (
            <div className="mt-3 flex justify-end">
              <Button
                size="sm"
                variant="secondary"
                disabled={setRelays.isPending || draft === null}
                onClick={() => {
                  setRelays.mutate(
                    text
                      .split("\n")
                      .map((line) => line.trim())
                      .filter(Boolean),
                    {
                      onSuccess: () => {
                        setDraft(null);
                        toast.success({ title: t("groupsPage.relaysSaved") });
                      },
                      onError: (error) => toast.error({ title: String(error) }),
                    },
                  );
                }}
              >
                {t("groupsPage.saveRelays")}
              </Button>
            </div>
          ) : null}
        </div>
      ) : null}

      <div className="mt-4 border-t border-border-subtle pt-4">
        <h3 className="text-sm font-medium text-text-primary">{t("groupsPage.groupFileLabel")}</h3>
        <p className="mt-1 text-sm text-text-secondary">{t("groupsPage.groupFileHint")}</p>
        <div className="mt-3 flex flex-wrap gap-2">
          <Button size="sm" variant="secondary" disabled={exportFile.isPending} onClick={onExport}>
            {t("groupsPage.exportGroup")}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={importFile.isPending}
            onClick={() => fileInput.current?.click()}
          >
            {t("groupsPage.importGroup")}
          </Button>
          <input
            ref={fileInput}
            type="file"
            accept="application/json,.json"
            className="hidden"
            aria-label={t("groupsPage.importGroup")}
            onChange={(event) => {
              onFilePicked(event.target.files?.[0]);
              event.target.value = "";
            }}
          />
        </div>
      </div>
    </section>
  );
}
