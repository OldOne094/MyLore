/* MISSION-071 — Export data layer. The user picks a format and a destination
   through the native save dialog (`@tauri-apps/plugin-dialog`), then the export
   runs as a background task: `export_media` resolves with the queued snapshot,
   `task-changed` events stream progress, and the typed `ExportReport`
   (`{ format, total, path }`) lands in the task's `result`. The task itself can
   be cancelled (dropping any partial file) via the shared task layer. */

import { useMutation } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { export_media, type TaskSnapshot } from "@/api";
import { useTask } from "@/features/tasks/api";

export type ExportFormat = "json" | "csv" | "markdown";

export interface ExportTarget {
  format: ExportFormat;
  path: string;
}

export const EXPORT_FORMATS: ExportFormat[] = ["json", "csv", "markdown"];

export function formatExtension(format: ExportFormat): string {
  return format === "markdown" ? "md" : format;
}

/** Spawn the export as a background task (MISSION-071). Resolves with the
    queued snapshot; follow progress through `useExportTask`. */
export function useExportMedia() {
  return useMutation({
    mutationFn: (target: ExportTarget): Promise<TaskSnapshot> =>
      export_media({ format: target.format, path: target.path }),
  });
}

/** Live snapshot of the export task (delegates to the shared `useTask`). */
export function useExportTask(taskId: string | null) {
  return useTask(taskId);
}

/** Native save dialog. Resolves with the chosen path or null when cancelled. */
export function pickExportPath(format: ExportFormat, filterName: string): Promise<string | null> {
  const extension = formatExtension(format);
  return save({
    title: "mylore",
    defaultPath: `mylore-library.${extension}`,
    filters: [{ name: filterName, extensions: [extension] }],
  });
}
