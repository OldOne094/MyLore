import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  Skeleton,
  useToast,
} from "@/components/ui";
import { backup_validate, type BackupEntry, type BackupReport } from "@/api";
import { cn } from "@/lib/cn";
import {
  useBackupCreate,
  useBackupDelete,
  useBackupList,
  useBackupPrefs,
  useBackupRestore,
  useBackupTask,
  useSaveBackupPrefs,
} from "./backups";

/* MISSION-088 — Backups section (settings page). Preferences for the
   automatic schedule + retention (saved on change), a "back up now" action
   that runs as a background task, and the archive list with per-archive
   validate / restore / delete. Restoring replaces the live library, so it is
   guarded by a confirm dialog and ends with a restart note. */

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(Math.round((bytes / (1024 * 1024)) * 10) / 10).toFixed(1)} MB`;
}

function formatDate(stamp: string): string {
  if (stamp.length !== 14) return stamp;
  return `${stamp.slice(0, 4)}-${stamp.slice(4, 6)}-${stamp.slice(6, 8)} ${stamp.slice(8, 10)}:${stamp.slice(10, 12)}`;
}

function baseName(report: BackupReport): string {
  return report.path.split(/[\\/]/).pop() ?? report.path;
}

const INTERVAL_CHOICES = [12, 24, 48, 168];

export function BackupsSection() {
  const { t } = useTranslation();
  const toast = useToast();
  const listQuery = useBackupList();
  const prefsQuery = useBackupPrefs();
  const savePrefs = useSaveBackupPrefs();
  const deleteArchive = useBackupDelete();
  const createBackup = useBackupCreate();
  const restoreMutation = useBackupRestore();

  const [createTaskId, setCreateTaskId] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState<string | null>(null);
  const [validity, setValidity] = useState<Record<string, "checking" | "ok" | "bad">>({});
  const [restoreTarget, setRestoreTarget] = useState<BackupEntry | null>(null);
  const [restoreTaskId, setRestoreTaskId] = useState<string | null>(null);

  const createTask = useBackupTask(createTaskId).data;
  const restoreTask = useBackupTask(restoreTaskId).data;
  const restoreReport =
    restoreTask?.state === "success" && restoreTask.result
      ? (restoreTask.result as BackupReport)
      : null;

  const prefs = prefsQuery.data;
  const entries = listQuery.data ?? [];
  const creating = createTaskId !== null && createTask?.state !== "failed";

  const runCreate = () => {
    if (creating) return;
    createBackup.mutate(undefined, {
      onSuccess: (snapshot) => setCreateTaskId(snapshot.id),
      onError: () => toast.error({ title: t("settings.backupsCreateFailed") }),
    });
  };

  const checkArchive = async (entry: BackupEntry) => {
    setValidity((prev) => ({ ...prev, [entry.path]: "checking" }));
    try {
      await backup_validate({ path: entry.path });
      setValidity((prev) => ({ ...prev, [entry.path]: "ok" }));
    } catch {
      setValidity((prev) => ({ ...prev, [entry.path]: "bad" }));
    }
  };

  const closeRestoreDialog = () => {
    setRestoreTarget(null);
    setRestoreTaskId(null);
  };

  const runRestore = () => {
    if (!restoreTarget || restoreTaskId) return;
    restoreMutation.mutate(restoreTarget.path, {
      onSuccess: (snapshot) => setRestoreTaskId(snapshot.id),
      onError: () => toast.error({ title: t("settings.backupsRestoreFailed") }),
    });
  };

  return (
    <section className="rounded-md border border-border-subtle bg-bg-surface p-6">
      <h2 className="text-sm font-semibold text-text-primary">{t("settings.backups")}</h2>
      <p className="mt-1 text-sm text-text-secondary">{t("settings.backupsHint")}</p>

      <div className="mt-4 flex flex-col gap-4">
        {prefs ? (
          <div className="flex flex-wrap items-end gap-4">
            <div
              role="group"
              aria-label={t("settings.backupsAuto")}
              className="inline-flex items-center gap-1 rounded-full border border-border-subtle bg-bg-raised p-1"
            >
              {[false, true].map((value) => (
                <button
                  key={String(value)}
                  type="button"
                  className={cn(
                    "rounded-full border-none bg-transparent px-3 py-1 text-sm text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary",
                    prefs.auto_enabled === value && "bg-accent text-bg-surface hover:bg-accent",
                  )}
                  aria-pressed={prefs.auto_enabled === value}
                  onClick={() => savePrefs.mutate({ ...prefs, auto_enabled: value })}
                >
                  {value ? t("settings.backupsOn") : t("settings.backupsOff")}
                </button>
              ))}
            </div>
            {prefs.auto_enabled ? (
              <label className="flex items-center gap-2 text-sm text-text-secondary">
                {t("settings.backupsInterval")}
                <select
                  value={prefs.interval_hours}
                  onChange={(event) =>
                    savePrefs.mutate({
                      ...prefs,
                      interval_hours: Number(event.target.value),
                    })
                  }
                  className="rounded-md border border-border-strong bg-bg-surface px-2 py-1 text-sm text-text-primary focus:border-accent focus:outline-none"
                >
                  {INTERVAL_CHOICES.map((hours) => (
                    <option key={hours} value={hours}>
                      {t("settings.backupsHours", { count: hours })}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
            <label className="flex items-center gap-2 text-sm text-text-secondary">
              {t("settings.backupsKeep")}
              <input
                type="number"
                min={1}
                max={100}
                value={prefs.keep_count}
                onChange={(event) => {
                  const keep = Number(event.target.value);
                  if (keep >= 1 && keep <= 100) savePrefs.mutate({ ...prefs, keep_count: keep });
                }}
                className="w-16 rounded-md border border-border-strong bg-bg-surface px-2 py-1 text-sm tabular-nums text-text-primary focus:border-accent focus:outline-none"
                aria-label={t("settings.backupsKeep")}
              />
            </label>
          </div>
        ) : (
          <Skeleton className="h-8 w-64" />
        )}

        <div className="flex items-center gap-3">
          <Button onClick={runCreate} disabled={creating}>
            {creating ? t("settings.backupsCreating") : t("settings.backupsCreate")}
          </Button>
          {createTask?.state === "success" && createTask.result ? (
            <span className="text-sm text-text-secondary" role="status">
              {t("settings.backupsCreated", {
                name: baseName(createTask.result as BackupReport),
              })}
            </span>
          ) : null}
        </div>
        {createTask?.state === "failed" ? (
          <p className="text-sm text-text-secondary">{createTask.error}</p>
        ) : null}

        <div>
          <h3 className="text-xs font-medium text-text-secondary">
            {t("settings.backupsArchives")}
          </h3>
          {listQuery.isLoading ? (
            <Skeleton className="mt-2 h-16" />
          ) : entries.length === 0 ? (
            <p className="mt-2 text-sm text-text-tertiary">{t("settings.backupsEmpty")}</p>
          ) : (
            <ul className="mt-2 flex flex-col divide-y divide-border-subtle">
              {entries.map((entry) => (
                <li key={entry.path} className="flex flex-wrap items-center gap-2 py-2 text-sm">
                  <span className="min-w-0 flex-1 truncate text-text-primary">
                    {entry.file_name}
                  </span>
                  <span className="tabular-nums text-text-tertiary">
                    {formatDate(entry.created_at)}
                  </span>
                  <span className="w-16 text-end tabular-nums text-text-tertiary">
                    {formatBytes(entry.size_bytes)}
                  </span>
                  <span
                    className={cn(
                      "w-10 text-end text-xs",
                      validity[entry.path] === "bad" && "text-destructive",
                    )}
                  >
                    {validity[entry.path] === "ok"
                      ? t("settings.backupsValid")
                      : validity[entry.path] === "bad"
                        ? t("settings.backupsInvalid")
                        : ""}
                  </span>
                  <Button variant="secondary" onClick={() => void checkArchive(entry)}>
                    {validity[entry.path] === "checking"
                      ? t("settings.backupsChecking")
                      : t("settings.backupsCheck")}
                  </Button>
                  <Button variant="secondary" onClick={() => setRestoreTarget(entry)}>
                    {t("settings.backupsRestore")}
                  </Button>
                  {confirmingDelete === entry.path ? (
                    <>
                      <Button
                        variant="secondary"
                        onClick={() => {
                          deleteArchive.mutate(entry.path, {
                            onError: () =>
                              toast.error({ title: t("settings.backupsDeleteFailed") }),
                          });
                          setConfirmingDelete(null);
                        }}
                      >
                        {t("settings.backupsConfirmDelete")}
                      </Button>
                      <Button variant="secondary" onClick={() => setConfirmingDelete(null)}>
                        {t("settings.backupsCancelDelete")}
                      </Button>
                    </>
                  ) : (
                    <Button variant="secondary" onClick={() => setConfirmingDelete(entry.path)}>
                      {t("settings.backupsDelete")}
                    </Button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      <Dialog open={restoreTarget !== null} onOpenChange={(open) => !open && closeRestoreDialog()}>
        <DialogContent closeLabel={t("settings.backupsClose")}>
          <DialogTitle>{t("settings.backupsRestoreTitle")}</DialogTitle>
          <DialogDescription>{t("settings.backupsRestoreHint")}</DialogDescription>
          <div className="mt-5 flex flex-col gap-4 text-sm">
            {restoreTarget ? <p className="text-text-primary">{restoreTarget.file_name}</p> : null}
            {restoreReport ? (
              <p className="text-text-secondary" role="status">
                {t("settings.backupsRestoreFinished")}
              </p>
            ) : restoreTask?.state === "failed" ? (
              <p className="text-text-secondary">{restoreTask.error}</p>
            ) : restoreTaskId ? (
              <p className="text-text-secondary" role="status">
                {t("settings.backupsRestoring")}
              </p>
            ) : null}
            <div className="mt-2 flex justify-end gap-2">
              {restoreReport || restoreTask?.state === "failed" ? (
                <DialogClose asChild>
                  <Button>{t("settings.backupsClose")}</Button>
                </DialogClose>
              ) : restoreTaskId ? null : (
                <>
                  <DialogClose asChild>
                    <Button variant="secondary">{t("settings.backupsCancelDelete")}</Button>
                  </DialogClose>
                  <Button onClick={runRestore}>{t("settings.backupsRestoreConfirm")}</Button>
                </>
              )}
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </section>
  );
}
