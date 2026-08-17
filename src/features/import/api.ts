/* MISSION-068 — File-import data layer. The dialog picks a file in the
   webview (FileReader), then commits through the MISSION-067 pipeline:
   JSON (app format) directly, CSV after a column mapping. The confirm/preview
   screen is MISSION-069, so the commit plan is null → "import every new row". */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { import_commit, import_csv_headers, type CsvMapping, type ImportReport } from "@/api";
import { queryKeys } from "@/api";

export type ImportFileKind = "json" | "csv";

export interface ImportFileTarget {
  kind: ImportFileKind;
  source: string;
  mapping: CsvMapping | null;
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

export function useImportFile() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (target: ImportFileTarget): Promise<ImportReport> =>
      import_commit({
        kind: target.kind,
        source: target.source,
        mapping: target.mapping,
        plan: null,
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
