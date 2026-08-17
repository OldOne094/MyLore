/* MISSION-068/069 — File-import data layer. The dialog picks a file in the
   webview (FileReader), previews it through `import_file_preview`, lets the
   user pick which new rows to import, then commits the selected rows
   (MISSION-069) through the MISSION-067 savepoint transaction. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  import_commit,
  import_csv_headers,
  import_file_preview,
  type CsvMapping,
  type ImportPlan,
  type ImportReport,
} from "@/api";
import { queryKeys } from "@/api";

export type ImportFileKind = "json" | "csv";

export interface ImportFileTarget {
  kind: ImportFileKind;
  source: string;
  mapping: CsvMapping | null;
  plan: ImportPlan | null;
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

export function useImportFile() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (target: ImportFileTarget): Promise<ImportReport> =>
      import_commit({
        kind: target.kind,
        source: target.source,
        mapping: target.mapping,
        plan: target.plan,
      }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.media.lists() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.media.facets() }),
        queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.all() }),
      ]);
    },
  });
}
