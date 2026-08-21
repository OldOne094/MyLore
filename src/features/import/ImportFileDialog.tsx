import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useTranslation } from "react-i18next";
import {
  Badge,
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
  useToast,
} from "@/components/ui";
import { type CsvMapping, type ImportPlan, type ImportReport, type PreviewItem } from "@/api";
import {
  PROFILE_KINDS,
  useCsvHeaders,
  useImportDetect,
  useImportFile,
  useImportPreview,
  useImportTask,
  type ImportFileKind,
} from "./api";
import { useTaskCancel } from "@/features/tasks/api";

/* MISSION-068/069/070/072 — Import-from-file dialog. Flow: pick a JSON/CSV file
   in the webview (FileReader) → the file is sniffed through `import_file_detect`
   (MISSION-072): profile exports (AniList/Goodreads/StoryGraph) show a badge
   and go straight to the preview with their built-in user state, while plain
   CSV opens the column-mapping table and JSON imports directly → the file is
   analyzed through `import_file_preview` and the per-item outcomes are shown
   (MISSION-069): check the new rows you want, then confirm or cancel. Confirm
   spawns a background task (MISSION-070): the dialog streams `task-changed`
   progress, can cancel, and shows the MISSION-067 savepoint report once the
   task succeeds. */

const SELECT_CLASSES =
  "h-[var(--control-height)] w-full rounded-sm border bg-bg-base px-3 text-base text-text-primary " +
  "transition-colors duration-150 ease-out hover:border-accent focus-visible:outline-none";

const CSV_DELIMITERS = [
  { value: ",", label: "Comma (,)" },
  { value: "\t", label: "Tab (\\t)" },
  { value: ";", label: "Semicolon (;)" },
  { value: "|", label: "Pipe (|)" },
];

const CONTENT_TYPES = [
  "book",
  "novel",
  "web_novel",
  "manga",
  "manhwa",
  "manhua",
  "anime",
  "tv",
  "movie",
  "other",
];

const PREVIEW_ROW_SIZE = 48;

type ColumnField = Exclude<keyof CsvMapping, "delimiter" | "separator">;

interface MappingRow {
  field: ColumnField;
  labelKey: string;
  hintKey?: string;
}

const MAPPING_ROWS: MappingRow[] = [
  { field: "title", labelKey: "import.fieldTitle" },
  { field: "title_original", labelKey: "import.fieldTitleOriginal" },
  { field: "alt_titles", labelKey: "import.fieldAltTitles" },
  { field: "content_type", labelKey: "import.fieldContentType" },
  { field: "format", labelKey: "import.fieldFormat" },
  { field: "pub_status", labelKey: "import.fieldPubStatus" },
  { field: "start_date", labelKey: "import.fieldStartDate" },
  { field: "end_date", labelKey: "import.fieldEndDate" },
  { field: "release_year", labelKey: "import.fieldReleaseYear" },
  { field: "language", labelKey: "import.fieldLanguage" },
  { field: "country", labelKey: "import.fieldCountry" },
  { field: "content_rating", labelKey: "import.fieldContentRating" },
  { field: "pages", labelKey: "import.fieldPages" },
  { field: "duration_min", labelKey: "import.fieldDuration" },
  { field: "ep_count", labelKey: "import.fieldEpisodes" },
  { field: "ch_count", labelKey: "import.fieldChapters" },
  { field: "synopsis", labelKey: "import.fieldSynopsis" },
  { field: "author", labelKey: "import.fieldAuthor" },
  { field: "artist", labelKey: "import.fieldArtist" },
  { field: "studio", labelKey: "import.fieldStudio" },
  { field: "genres", labelKey: "import.fieldGenres" },
  { field: "tags", labelKey: "import.fieldTags" },
  { field: "external_id", labelKey: "import.fieldExternalId", hintKey: "import.externalIdHint" },
  { field: "cover_url", labelKey: "import.fieldCoverUrl" },
  { field: "banner_url", labelKey: "import.fieldBannerUrl" },
];

function defaultMapping(): CsvMapping {
  return {
    title: null,
    title_original: null,
    alt_titles: null,
    content_type: null,
    default_content_type: null,
    format: null,
    pub_status: null,
    start_date: null,
    end_date: null,
    release_year: null,
    language: null,
    country: null,
    content_rating: null,
    pages: null,
    duration_min: null,
    ep_count: null,
    ch_count: null,
    synopsis: null,
    author: null,
    artist: null,
    studio: null,
    genres: null,
    tags: null,
    external_id: null,
    cover_url: null,
    banner_url: null,
    delimiter: ",",
    separator: ",",
  };
}

const OUTCOME_LABEL_KEYS: Record<PreviewItem["outcome"], string> = {
  new: "import.outcomeNew",
  in_library: "import.outcomeInLibrary",
  duplicate: "import.outcomeDuplicate",
  invalid: "import.outcomeInvalid",
};

const OUTCOME_BADGE_VARIANTS: Record<
  PreviewItem["outcome"],
  "accent" | "completed" | "onhold" | "dropped"
> = {
  new: "accent",
  in_library: "completed",
  duplicate: "onhold",
  invalid: "dropped",
};

const PROFILE_LABEL_KEYS: Record<string, string> = {
  anilist: "import.profileAniList",
  goodreads: "import.profileGoodreads",
  storygraph: "import.profileStorygraph",
};

export interface ImportFileDialogProps {
  trigger: React.ReactNode;
}

export function ImportFileDialog({ trigger }: ImportFileDialogProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const importFile = useImportFile();
  const taskCancel = useTaskCancel();
  const fileInput = useRef<HTMLInputElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const selectAllRef = useRef<HTMLInputElement>(null);

  const [open, setOpen] = useState(false);
  const [fileName, setFileName] = useState<string | null>(null);
  const [source, setSource] = useState<string | null>(null);
  const [kind, setKind] = useState<ImportFileKind | null>(null);
  const [delimiter, setDelimiter] = useState(",");
  const [separator, setSeparator] = useState(",");
  const [mapping, setMapping] = useState<CsvMapping>(defaultMapping);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());

  const detectQuery = useImportDetect(source ?? "", source !== null);
  useEffect(() => {
    if (detectQuery.data) setKind(detectQuery.data as ImportFileKind);
  }, [detectQuery.data]);

  const taskQuery = useImportTask(taskId);
  const task = taskQuery.data;
  const report = task?.state === "success" && task.result ? (task.result as ImportReport) : null;
  const importing =
    taskId !== null && !report && task?.state !== "failed" && task?.state !== "cancelled";

  const openDialog = (value: boolean) => {
    setOpen(value);
    if (value) {
      setFileName(null);
      setSource(null);
      setKind(null);
      setDelimiter(",");
      setSeparator(",");
      setMapping(defaultMapping());
      setTaskId(null);
      setSelected(new Set());
      if (fileInput.current) fileInput.current.value = "";
    }
  };

  const onFile = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const text = typeof reader.result === "string" ? reader.result : "";
      setFileName(file.name);
      setSource(text);
      setKind(null);
      setTaskId(null);
    };
    reader.readAsText(file);
  };

  const titleMapped = kind !== "csv" || mapping.title != null;
  const effectiveMapping = useMemo<CsvMapping | null>(
    () => (kind === "csv" ? { ...mapping, delimiter, separator } : null),
    [kind, mapping, delimiter, separator],
  );

  const headersQuery = useCsvHeaders(source ?? "", delimiter, kind === "csv" && source !== null);
  const columns = headersQuery.data ?? [];

  const showPreview = kind !== null && source !== null && titleMapped;
  const previewQuery = useImportPreview(
    kind ?? "json",
    source ?? "",
    effectiveMapping,
    showPreview,
  );
  const preview = previewQuery.data;

  const items = preview?.items ?? [];
  const rowVirtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => PREVIEW_ROW_SIZE,
    overscan: 8,
  });

  const newRows = useMemo(
    () =>
      preview
        ? preview.items.filter((item) => item.outcome === "new").map((item) => item.source_row)
        : [],
    [preview],
  );
  const allSelected = newRows.length > 0 && selected.size === newRows.length;
  const someSelected = selected.size > 0 && !allSelected;

  useEffect(() => {
    if (!preview) return;
    setSelected(new Set(newRows));
  }, [preview, newRows]);

  useEffect(() => {
    if (selectAllRef.current) selectAllRef.current.indeterminate = someSelected;
  }, [someSelected]);

  const toggleRow = (row: number) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(row)) {
        next.delete(row);
      } else {
        next.add(row);
      }
      return next;
    });
  };

  const toggleAll = () => {
    setSelected(allSelected ? new Set() : new Set(newRows));
  };

  const plan = useMemo<ImportPlan | null>(
    () => ({ rows: [...selected].sort((a, b) => a - b) }),
    [selected],
  );

  const setColumn = (field: ColumnField, value: string) => {
    setMapping((previous) => ({ ...previous, [field]: value || null }));
  };

  const canImport =
    showPreview &&
    preview !== undefined &&
    !previewQuery.isError &&
    selected.size > 0 &&
    !importFile.isPending;

  const runImport = () => {
    if (!source || !kind || !canImport) return;
    importFile.mutate(
      { kind, source, mapping: effectiveMapping, plan },
      {
        onSuccess: (snapshot) => setTaskId(snapshot.id),
        onError: () => toast.error({ title: t("import.errorTitle") }),
      },
    );
  };

  const cancelTask = () => {
    if (taskId) {
      taskCancel.mutate(taskId, {
        onError: () => toast.error({ title: t("import.cancelFailed") }),
      });
    }
  };

  return (
    <Dialog open={open} onOpenChange={openDialog}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent closeLabel={t("import.close")}>
        <DialogTitle>{t("import.dialogTitle")}</DialogTitle>
        <DialogDescription>{t("import.dialogHint")}</DialogDescription>

        <div className="mt-5 flex flex-col gap-4">
          <div>
            <input
              ref={fileInput}
              type="file"
              accept=".json,.csv,application/json,text/csv"
              className="hidden"
              onChange={onFile}
              aria-label={t("import.chooseFile")}
            />
            <Button variant="secondary" onClick={() => fileInput.current?.click()}>
              {t("import.chooseFile")}
            </Button>
            {fileName ? (
              <p className="mt-2 truncate text-sm text-text-secondary">{fileName}</p>
            ) : (
              <p className="mt-2 text-sm text-text-tertiary">{t("import.fileHint")}</p>
            )}
          </div>

          {source === null ? (
            <p className="text-sm text-text-tertiary">{t("import.fileHint")}</p>
          ) : detectQuery.isPending ? (
            <p className="text-sm text-text-secondary">{t("import.detecting")}</p>
          ) : kind === null ? null : PROFILE_KINDS.has(kind) ? (
            <div className="flex flex-col gap-1.5">
              <Badge variant="accent" className="w-fit">
                {t(PROFILE_LABEL_KEYS[kind])}
              </Badge>
              <p className="text-sm text-text-tertiary">{t("import.profileHint")}</p>
            </div>
          ) : kind === "csv" ? (
            <div className="flex flex-col gap-3">
              <p className="text-sm font-medium text-text-primary">{t("import.csvStep")}</p>
              <div className="grid grid-cols-2 gap-3">
                <label className="flex flex-col gap-1.5">
                  <span className="text-sm font-medium text-text-secondary">
                    {t("import.delimiter")}
                  </span>
                  <select
                    className={SELECT_CLASSES}
                    aria-label={t("import.delimiter")}
                    value={delimiter}
                    onChange={(event) => setDelimiter(event.target.value)}
                  >
                    {CSV_DELIMITERS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex flex-col gap-1.5">
                  <span className="text-sm font-medium text-text-secondary">
                    {t("import.separator")}
                  </span>
                  <input
                    className={SELECT_CLASSES}
                    aria-label={t("import.separator")}
                    value={separator}
                    onChange={(event) => setSeparator(event.target.value)}
                  />
                </label>
              </div>

              {headersQuery.isLoading ? (
                <p className="text-sm text-text-tertiary">{t("import.readingHeaders")}</p>
              ) : headersQuery.isError || columns.length === 0 ? (
                <p className="text-sm text-text-tertiary">{t("import.noColumns")}</p>
              ) : (
                <div className="max-h-72 overflow-y-auto rounded-sm border border-border-subtle">
                  <table className="w-full text-start text-sm">
                    <thead className="sticky top-0 bg-bg-surface">
                      <tr>
                        <th className="px-3 py-2 font-medium text-text-secondary">
                          {t("import.fieldLabel")}
                        </th>
                        <th className="px-3 py-2 font-medium text-text-secondary">
                          {t("import.columnLabel")}
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {MAPPING_ROWS.map((row) => (
                        <tr key={row.field} className="border-t border-border-subtle">
                          <td className="px-3 py-1.5 text-text-primary">
                            {t(row.labelKey)}
                            {row.hintKey ? (
                              <span className="ms-1 text-xs text-text-tertiary">
                                — {t(row.hintKey)}
                              </span>
                            ) : null}
                          </td>
                          <td className="px-3 py-1.5">
                            <select
                              className={SELECT_CLASSES}
                              aria-label={t(row.labelKey)}
                              value={mapping[row.field] ?? ""}
                              onChange={(event) => setColumn(row.field, event.target.value)}
                            >
                              <option value="">{t("import.unmapped")}</option>
                              {columns.map((column) => (
                                <option key={column} value={column}>
                                  {column}
                                </option>
                              ))}
                            </select>
                          </td>
                        </tr>
                      ))}
                      <tr className="border-t border-border-subtle">
                        <td className="px-3 py-1.5 text-text-primary">
                          {t("import.fieldDefaultType")}
                        </td>
                        <td className="px-3 py-1.5">
                          <select
                            className={SELECT_CLASSES}
                            aria-label={t("import.fieldDefaultType")}
                            value={mapping.default_content_type ?? ""}
                            onChange={(event) =>
                              setMapping((previous) => ({
                                ...previous,
                                default_content_type: event.target.value || null,
                              }))
                            }
                          >
                            <option value="">{t("import.defaultTypeNone")}</option>
                            {CONTENT_TYPES.map((type) => (
                              <option key={type} value={type}>
                                {t(`contentType.${type}`)}
                              </option>
                            ))}
                          </select>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              )}

              {!titleMapped ? (
                <p className="text-sm text-text-tertiary">{t("import.titleRequired")}</p>
              ) : null}
            </div>
          ) : null}

          {showPreview ? (
            previewQuery.isPending ? (
              <p className="text-sm text-text-secondary">{t("import.analyzing")}</p>
            ) : previewQuery.isError ? (
              <div className="flex flex-col gap-2">
                <p className="text-sm text-text-tertiary">{t("import.previewError")}</p>
                <Button
                  variant="secondary"
                  onClick={() => void previewQuery.refetch()}
                  className="w-fit"
                >
                  {t("import.retry")}
                </Button>
              </div>
            ) : preview ? (
              <div className="flex flex-col gap-3">
                <p className="text-sm font-medium text-text-primary">{t("import.previewTitle")}</p>
                <div className="flex flex-wrap items-center gap-2 text-sm text-text-secondary">
                  <Badge variant="accent">{t("import.sumNew", { count: preview.new })}</Badge>
                  <Badge>{t("import.sumInLibrary", { count: preview.in_library })}</Badge>
                  <Badge variant="onhold">
                    {t("import.sumDuplicates", { count: preview.duplicates })}
                  </Badge>
                  <Badge variant="dropped">
                    {t("import.sumInvalid", { count: preview.invalid })}
                  </Badge>
                </div>

                {newRows.length === 0 ? (
                  <p className="text-sm text-text-tertiary">{t("import.nothingNew")}</p>
                ) : (
                  <>
                    <label className="flex items-center gap-2 text-sm text-text-secondary">
                      <input
                        ref={selectAllRef}
                        type="checkbox"
                        checked={allSelected}
                        onChange={toggleAll}
                        aria-label={t("import.selectAllNew")}
                      />
                      {t("import.selectAllNew")}
                    </label>
                    <div
                      ref={scrollRef}
                      role="list"
                      data-import-preview=""
                      className="max-h-72 overflow-y-auto rounded-sm border border-border-subtle"
                    >
                      <div
                        className="relative w-full"
                        style={{ height: rowVirtualizer.getTotalSize() }}
                      >
                        {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                          const item = items[virtualRow.index];
                          const selectable = item.outcome === "new";
                          const issue = item.issues[0];
                          return (
                            <div
                              key={item.source_row}
                              className="absolute inset-x-0 top-0 flex items-center gap-3 border-b border-border-subtle px-3 last:border-b-0"
                              style={{ top: virtualRow.start, height: virtualRow.size }}
                            >
                              <input
                                type="checkbox"
                                checked={selected.has(item.source_row)}
                                disabled={!selectable}
                                onChange={() => toggleRow(item.source_row)}
                                aria-label={t("import.selectRowAria", { row: item.source_row })}
                              />
                              <span className="w-12 shrink-0 text-xs text-text-tertiary">
                                {t("import.rowPrefix", { row: item.source_row })}
                              </span>
                              <span className="min-w-0 flex-1 truncate text-sm text-text-primary">
                                {item.title ?? t("import.untitled")}
                                {issue ? (
                                  <span className="text-xs text-text-tertiary">
                                    {" "}
                                    — {issue.message}
                                  </span>
                                ) : null}
                              </span>
                              <Badge variant={OUTCOME_BADGE_VARIANTS[item.outcome]}>
                                {t(OUTCOME_LABEL_KEYS[item.outcome])}
                              </Badge>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  </>
                )}
              </div>
            ) : null
          ) : null}

          {report ? (
            <div className="rounded-sm border border-border-subtle p-3 text-sm">
              <p className="font-medium text-text-primary">{t("import.resultTitle")}</p>
              <p className="mt-1 text-text-secondary">
                {t("import.resultCommitted", { count: report.committed })} ·{" "}
                {t("import.resultSkipped", { count: report.skipped })} ·{" "}
                {t("import.resultFailed", { count: report.failed })}
              </p>
            </div>
          ) : importing ? (
            <div
              role="status"
              aria-live="polite"
              className="flex flex-col gap-2 rounded-sm border border-border-subtle p-3 text-sm"
            >
              <p className="font-medium text-text-primary">{t("import.importing")}</p>
              {task?.message ? <p className="text-text-secondary">{task.message}</p> : null}
              <div className="h-1.5 w-full overflow-hidden rounded-full bg-bg-hover">
                <div
                  className="h-full bg-accent transition-[width] duration-150 ease-out"
                  style={{ width: `${task?.progress ?? 0}%` }}
                />
              </div>
            </div>
          ) : task?.state === "failed" ? (
            <div className="rounded-sm border border-border-subtle p-3 text-sm">
              <p className="font-medium text-text-primary">{t("import.errorTitle")}</p>
              {task.error ? <p className="mt-1 text-text-secondary">{task.error}</p> : null}
            </div>
          ) : task?.state === "cancelled" ? (
            <div className="rounded-sm border border-border-subtle p-3 text-sm">
              <p className="font-medium text-text-primary">{t("import.importCancelled")}</p>
            </div>
          ) : null}

          <div className="mt-2 flex justify-end gap-2">
            {report ? (
              <DialogClose asChild>
                <Button>{t("import.close")}</Button>
              </DialogClose>
            ) : importing ? (
              <>
                <DialogClose asChild>
                  <Button variant="secondary">{t("import.close")}</Button>
                </DialogClose>
                <Button variant="secondary" onClick={cancelTask} disabled={taskCancel.isPending}>
                  {t("import.cancelTask")}
                </Button>
              </>
            ) : task?.state === "failed" || task?.state === "cancelled" ? (
              <DialogClose asChild>
                <Button>{t("import.close")}</Button>
              </DialogClose>
            ) : (
              <>
                <DialogClose asChild>
                  <Button variant="secondary">{t("import.cancel")}</Button>
                </DialogClose>
                <Button onClick={runImport} disabled={!canImport}>
                  {importFile.isPending
                    ? t("import.importing")
                    : t("import.importSelected", { count: selected.size })}
                </Button>
              </>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
