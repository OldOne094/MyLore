import { useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { Button, useToast } from "@/components/ui";
import type { BackupEntry } from "@/api";
import { useAppHealth, useRecoverRestore, useRecoverStartFresh } from "./api";
import { useBackupList } from "@/features/settings/backups";

/* MISSION-088 — Recovery screen. Shown instead of the whole shell when the
   database failed its startup integrity check. Offers the same two exits the
   roadmap calls for: restore a `.mylore` archive (from the backups folder or
   any file via the native dialog), or move the corrupt database aside and
   start fresh. Both close the pool, so every path ends in a restart note. */

function RecoveryEntry({
  entry,
  onRestore,
}: {
  entry: BackupEntry;
  onRestore: (path: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <li className="flex items-center gap-3 py-2 text-sm">
      <span className="min-w-0 flex-1 truncate text-text-primary">{entry.file_name}</span>
      <Button variant="secondary" onClick={() => onRestore(entry.path)}>
        {t("recovery.restoreAction")}
      </Button>
    </li>
  );
}

export function RecoveryScreen() {
  const { t } = useTranslation();
  const toast = useToast();
  const listQuery = useBackupList();
  const restore = useRecoverRestore();
  const startFresh = useRecoverStartFresh();

  // Set once any recovery action succeeds; the app must be restarted.
  const [outcome, setOutcome] = useState<string | null>(null);
  const [confirmingFresh, setConfirmingFresh] = useState(false);

  const runRestore = (path: string) => {
    if (restore.isPending || outcome) return;
    restore.mutate(path, {
      onSuccess: (result) => setOutcome(result.quarantined_to),
      onError: () => toast.error({ title: t("recovery.restoreFailed") }),
    });
  };

  const pickFile = async () => {
    const path = await open({
      title: t("recovery.chooseFile"),
      multiple: false,
      filters: [{ name: "MyLore backup", extensions: ["mylore"] }],
    });
    if (typeof path === "string") runRestore(path);
  };

  const runStartFresh = () => {
    if (startFresh.isPending || outcome) return;
    startFresh.mutate(undefined, {
      onSuccess: (result) => setOutcome(result.quarantined_to),
      onError: () => toast.error({ title: t("recovery.startFreshFailed") }),
    });
  };

  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <div className="w-full max-w-lg rounded-md border border-border-subtle bg-bg-surface p-6">
        <h1 className="text-lg font-semibold text-text-primary">{t("recovery.title")}</h1>
        <p className="mt-2 text-sm text-text-secondary">{t("recovery.hint")}</p>

        {outcome ? (
          <div className="mt-5 rounded-sm border border-border-subtle p-4 text-sm" role="status">
            <p className="font-medium text-text-primary">{t("recovery.restartRequired")}</p>
            <p className="mt-1 text-text-tertiary">
              {t("recovery.quarantined", { name: outcome.split(/[\\/]/).pop() ?? outcome })}
            </p>
          </div>
        ) : (
          <>
            <div className="mt-5 flex flex-col gap-2">
              <Button onClick={() => void pickFile()}>{t("recovery.chooseFile")}</Button>
              {confirmingFresh ? (
                <div className="flex gap-2">
                  <Button variant="secondary" onClick={runStartFresh}>
                    {t("recovery.startFreshConfirm")}
                  </Button>
                  <Button variant="secondary" onClick={() => setConfirmingFresh(false)}>
                    {t("recovery.cancel")}
                  </Button>
                </div>
              ) : (
                <Button variant="secondary" onClick={() => setConfirmingFresh(true)}>
                  {t("recovery.startFresh")}
                </Button>
              )}
            </div>

            {listQuery.data && listQuery.data.length > 0 ? (
              <div className="mt-6">
                <h2 className="text-xs font-medium text-text-secondary">
                  {t("recovery.archives")}
                </h2>
                <ul className="mt-1 flex flex-col divide-y divide-border-subtle">
                  {listQuery.data.map((entry) => (
                    <RecoveryEntry key={entry.path} entry={entry} onRestore={runRestore} />
                  ))}
                </ul>
              </div>
            ) : null}
          </>
        )}
      </div>
    </div>
  );
}

/** Route-level gate: renders the shell immediately and swaps it for the
    recovery screen only once startup explicitly reported an unhealthy
    database — never blocking the normal boot path on the health check. */
export function HealthGate({ children }: { children: React.ReactNode }) {
  const { data } = useAppHealth();
  if (data?.database_ok === false) return <RecoveryScreen />;
  return <>{children}</>;
}
