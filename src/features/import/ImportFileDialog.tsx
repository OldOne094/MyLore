import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
  useToast,
} from "@/components/ui";
import { type CsvMapping, type ImportReport } from "@/api";
import { useCsvHeaders, useImportFile, type ImportFileKind } from "./api";

/* MISSION-068 — Import-from-file dialog. Flow: pick a JSON/CSV file in the
   webview (FileReader) → JSON imports directly, CSV opens the column-mapping
   table → Import runs the MISSION-067 pipeline and shows the report
   (added / skipped / failed). The richer per-item preview + confirm screen is
   MISSION-069. */

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

export interface ImportFileDialogProps {
  trigger: React.ReactNode;
}

export function ImportFileDialog({ trigger }: ImportFileDialogProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const importFile = useImportFile();
  const fileInput = useRef<HTMLInputElement>(null);

  const [open, setOpen] = useState(false);
  const [fileName, setFileName] = useState<string | null>(null);
  const [source, setSource] = useState<string | null>(null);
  const [kind, setKind] = useState<ImportFileKind | null>(null);
  const [delimiter, setDelimiter] = useState(",");
  const [separator, setSeparator] = useState(",");
  const [mapping, setMapping] = useState<CsvMapping>(defaultMapping);
  const [report, setReport] = useState<ImportReport | null>(null);

  const openDialog = (value: boolean) => {
    setOpen(value);
    if (value) {
      setFileName(null);
      setSource(null);
      setKind(null);
      setDelimiter(",");
      setSeparator(",");
      setMapping(defaultMapping());
      setReport(null);
      if (fileInput.current) fileInput.current.value = "";
    }
  };

  const onFile = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const text = typeof reader.result === "string" ? reader.result : "";
      const isJson = file.name.toLowerCase().endsWith(".json");
      setFileName(file.name);
      setSource(text);
      setKind(isJson ? "json" : "csv");
      setReport(null);
    };
    reader.readAsText(file);
  };

  const headersQuery = useCsvHeaders(source ?? "", delimiter, kind === "csv" && source !== null);
  const columns = headersQuery.data ?? [];

  const setColumn = (field: ColumnField, value: string) => {
    setMapping((previous) => ({ ...previous, [field]: value || null }));
  };

  const titleMapped = kind !== "csv" || mapping.title != null;
  const canImport = source !== null && kind !== null && titleMapped && !importFile.isPending;
  const runImport = () => {
    if (!source || !kind || !titleMapped) return;
    importFile.mutate(
      { kind, source, mapping: kind === "csv" ? mapping : null },
      {
        onSuccess: setReport,
        onError: () => toast.error({ title: t("import.errorTitle") }),
      },
    );
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

          {source === null || kind === null ? null : kind === "json" ? (
            <p className="text-sm text-text-secondary">{t("import.jsonReady")}</p>
          ) : (
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
                  <table className="w-full text-left text-sm">
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
                              <span className="ml-1 text-xs text-text-tertiary">
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
          )}

          {report ? (
            <div className="rounded-sm border border-border-subtle p-3 text-sm">
              <p className="font-medium text-text-primary">{t("import.resultTitle")}</p>
              <p className="mt-1 text-text-secondary">
                {t("import.resultCommitted", { count: report.committed })} ·{" "}
                {t("import.resultSkipped", { count: report.skipped })} ·{" "}
                {t("import.resultFailed", { count: report.failed })}
              </p>
            </div>
          ) : null}

          <div className="mt-2 flex justify-end gap-2">
            {report ? (
              <DialogClose asChild>
                <Button>{t("import.close")}</Button>
              </DialogClose>
            ) : (
              <>
                <DialogClose asChild>
                  <Button variant="secondary">{t("import.cancel")}</Button>
                </DialogClose>
                <Button onClick={runImport} disabled={!canImport}>
                  {importFile.isPending ? t("import.importing") : t("import.import")}
                </Button>
              </>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
