/* MISSION-088 — Backups data layer. Listing/deleting archives and the backup
   preferences are plain queries; creating/restoring a backup runs as a
   background task (spawn + follow `task-changed` like the export flow). */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  backup_create,
  backup_delete,
  backup_list,
  backup_prefs_get,
  backup_prefs_set,
  backup_restore,
  type BackupEntry,
  type BackupPrefs,
} from "@/api";
import { queryKeys } from "@/api";
import { useTask } from "@/features/tasks/api";

/** Every archive in the backups folder, newest first. */
export function useBackupList() {
  return useQuery({
    queryKey: queryKeys.backups.list(),
    queryFn: () => backup_list(),
  });
}

/** Delete one archive (path re-derived server-side). */
export function useBackupDelete() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => backup_delete({ path }),
    onSuccess: () => client.invalidateQueries({ queryKey: queryKeys.backups.all() }),
  });
}

/** Backup preferences: automatic schedule + retention. */
export function useBackupPrefs() {
  return useQuery({
    queryKey: queryKeys.settings.backupPrefs(),
    queryFn: () => backup_prefs_get(),
  });
}

/** Validate and persist the backup preferences. */
export function useSaveBackupPrefs() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (prefs: BackupPrefs) =>
      backup_prefs_set({
        auto_enabled: prefs.auto_enabled,
        interval_hours: prefs.interval_hours,
        keep_count: prefs.keep_count,
      }),
    onSuccess: (prefs) => client.setQueryData(queryKeys.settings.backupPrefs(), prefs),
  });
}

/** Spawn a backup as a background task; follow with {@link useBackupTask}. */
export function useBackupCreate() {
  return useMutation({
    mutationFn: () => backup_create(),
  });
}

/** Restore an archive over the live library as a background task
    (rollback-safe; restart required on success). */
export function useBackupRestore() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => backup_restore({ path }),
    onSuccess: () => client.invalidateQueries({ queryKey: queryKeys.backups.all() }),
  });
}

/** Live snapshot of a running backup/restore task. */
export function useBackupTask(taskId: string | null) {
  return useTask(taskId);
}

export type { BackupEntry, BackupPrefs };
