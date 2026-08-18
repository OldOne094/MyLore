/* MISSION-068/069/070/072 — File-import data layer. The dialog picks a file in
   the webview (FileReader), sniffs its format through `import_file_detect`
   (MISSION-072: anilist/goodreads/storygraph profile exports skip the mapping
   step), previews it through `import_file_preview`, lets the user pick which
   new rows to import, then commits (MISSION-070) by spawning a background task:
   `import_commit` resolves with the queued snapshot and the `task-changed`
   event streams progress until the typed `ImportReport` lands in the task's
   `result`. The running dialog can cancel. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  import_commit,
  import_csv_headers,
  import_file_detect,
  import_file_preview,
  type CsvMapping,
  type ImportPlan,
  type TaskSnapshot,
} from "@/api";
import { queryKeys } from "@/api";
import { useTask } from "@/features/tasks/api";

export type ImportFileKind = "json" | "csv" | "anilist" | "goodreads" | "storygraph";

/** Kinds whose user list state (status/rating/progress) comes built in — no
    column-mapping step. */
export const PROFILE_KINDS: ReadonlySet<ImportFileKind> = new Set([
  "anilist",
  "goodreads",
  "storygraph",
]);

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

/** Sniff the file's import kind (MISSION-072). */
export function useImportDetect(source: string, enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.import.detect(source),
    queryFn: () => import_file_detect({ source }),
    enabled: enabled && source.length > 0,
    staleTime: Infinity,
    gcTime: Infinity,
  });
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

/** Live snapshot of the import task; on success the library/dashboard queries
    are invalidated. Delegates to the shared `useTask` hook. */
export function useImportTask(taskId: string | null) {
  const queryClient = useQueryClient();
  return useTask(taskId, {
    onSuccess: () => {
      void invalidateAfterImport(queryClient);
    },
  });
}
