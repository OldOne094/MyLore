/* MISSION-068/069/070 — File-import data layer. The dialog picks a file in the
   webview (FileReader), previews it through `import_file_preview`, lets the
   user pick which new rows to import, then commits (MISSION-070) by spawning a
   background task: `import_commit` resolves with the queued snapshot and the
   `task-changed` event streams progress until the typed `ImportReport` lands
   in the task's `result`. The running dialog can cancel. */

import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  import_commit,
  import_csv_headers,
  import_file_preview,
  listenTaskChanged,
  task_cancel,
  task_get,
  type CsvMapping,
  type ImportPlan,
  type TaskSnapshot,
} from "@/api";
import { queryKeys } from "@/api";

export type ImportFileKind = "json" | "csv";

export interface ImportFileTarget {
  kind: ImportFileKind;
  source: string;
  mapping: CsvMapping | null;
  plan: ImportPlan | null;
}

async function invalidateAfterImport(queryClient: ReturnType<typeof useQueryClient>) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.media.facets() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() }),
  ]);
}

export function useCsvHeaders(source: string, delimiter: string, enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.import.csvHeaders(source, delimiter),
    queryFn: () => import_csv_headers({ source, delimiter }),
    enabled: enabled && source.length > 0,
    staleTime: Infinity,
    gcTime: Infinity,
  });
}

/** Parse + dedup the file and return the per-item preview (MISSION-069). */
export function useImportPreview(
  kind: ImportFileKind,
  source: string,
  mapping: CsvMapping | null,
  enabled: boolean,
) {
  return useQuery({
    queryKey: queryKeys.import.preview(kind, source, mapping),
    queryFn: () => import_file_preview({ kind, source, mapping }),
    enabled: enabled && source.length > 0,
    retry: 1,
  });
}

/** Spawn the import as a background task (MISSION-070). Resolves with the
    queued snapshot; follow progress through `useImportTask`. */
export function useImportFile() {
  return useMutation({
    mutationFn: (target: ImportFileTarget): Promise<TaskSnapshot> =>
      import_commit({
        kind: target.kind,
        source: target.source,
        mapping: target.mapping,
        plan: target.plan,
      }),
  });
}

/** Request cancellation of a background task (MISSION-070). */
export function useTaskCancel() {
  return useMutation({
    mutationFn: (id: string) => task_cancel({ id }),
  });
}

/** Subscribe to one task's `task-changed` stream and expose its live snapshot.
    The dialog seeds the id from `import_commit`; `task_get` covers the case
    where an event is missed, and a success terminal state invalidates the
    library/dashboard queries. */
export function useImportTask(taskId: string | null) {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!taskId) return;
    let disposed = false;
    let unlisten: UnlistenFn | undefined;

    const sync = (snapshot: TaskSnapshot) => {
      queryClient.setQueryData(queryKeys.task.detail(taskId), snapshot);
      if (snapshot.state === "success") {
        void invalidateAfterImport(queryClient);
      }
    };

    void listenTaskChanged((snapshot) => {
      if (snapshot.id !== taskId) return;
      sync(snapshot);
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [taskId, queryClient]);

  return useQuery({
    queryKey: queryKeys.task.detail(taskId ?? ""),
    queryFn: () => task_get({ id: taskId! }),
    enabled: taskId !== null,
  });
}
