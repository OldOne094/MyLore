import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogContent,
  DialogTitle,
  InputField,
  TextareaField,
  useToast,
} from "@/components/ui";
import { PUBLICATION_STATUS_VALUES } from "./AddMediaSchema";
import {
  useMediaFacetsQuery,
  useMediaOverridesQuery,
  useSetMediaCover,
  useUpdateMedia,
  type MediaDetail,
} from "./api";

/* MISSION-161 — Correct a title after an import or a refresh.

   The fields sent are the full current values, so clearing one is explicit.
   Everything the user actually changes is **pinned** by the backend: a later
   provider refresh keeps it and reports that it did, instead of silently
   replacing the edit. A pinned field is marked here and can be released with a
   checkbox — the only way a refresh can take a field back. */

const SELECT_CLASSES =
  "h-[var(--control-height)] w-full rounded-sm border bg-bg-base px-3 text-base text-text-primary " +
  "transition-colors duration-150 ease-out hover:border-accent focus-visible:outline-none";

/** The fields a hand edit can pin — the same keys the backend and enrich use. */
const PINNABLE: { field: string; labelKey: string }[] = [
  { field: "title_main", labelKey: "library.fieldTitle" },
  { field: "pub_status", labelKey: "library.fieldStatus" },
  { field: "format", labelKey: "library.fieldFormat" },
  { field: "release_year", labelKey: "library.fieldYear" },
  { field: "synopsis", labelKey: "library.fieldSynopsis" },
  { field: "genres", labelKey: "library.fieldGenres" },
];

export interface EditMediaDialogProps {
  media: MediaDetail;
  onOpenChange: (open: boolean) => void;
}

export function EditMediaDialog({ media, onOpenChange }: EditMediaDialogProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const update = useUpdateMedia();
  const overridesQuery = useMediaOverridesQuery(media.id);
  const facets = useMediaFacetsQuery();
  const overrides = overridesQuery.data ?? [];

  const [title, setTitle] = useState(media.title_main);
  const [pubStatus, setPubStatus] = useState(media.pub_status);
  const [format, setFormat] = useState(media.format ?? "");
  const [releaseYear, setReleaseYear] = useState(
    media.release_year === null ? "" : String(media.release_year),
  );
  const [synopsis, setSynopsis] = useState(media.synopsis ?? "");
  /** The aggregate carries genre ids; the field edits names. */
  const [genres, setGenres] = useState(media.genres.map((id) => id.replace(/_/g, " ")).join(", "));
  const genresTouched = useRef(false);
  /* The facet list — the only place an id can be turned into its name — resolves
     after the first paint. Fill the field in when it lands, but never over
     something the user has already typed. */
  useEffect(() => {
    const names = new Map((facets.data?.genres ?? []).map((genre) => [genre.id, genre.name]));
    if (names.size === 0 || genresTouched.current) return;
    setGenres(media.genres.map((id) => names.get(id) ?? id.replace(/_/g, " ")).join(", "));
  }, [facets.data, media.genres]);
  const [release, setRelease] = useState<string[]>([]);
  const setCover = useSetMediaCover(media.id);
  const [coverUrl, setCoverUrl] = useState("");

  const toggleRelease = (field: string, checked: boolean) =>
    setRelease((current) =>
      checked ? [...current, field] : current.filter((entry) => entry !== field),
    );

  /* The cover is applied on its own, not with the form: it replaces an asset
     rather than a column, so it has its own button and its own result. */
  const applyCover = (url: string | null) => {
    setCover.mutate(url, {
      onSuccess: () =>
        toast.success({
          title: url ? t("library.coverSetToast") : t("library.coverClearedToast"),
        }),
      onError: () => toast.error({ title: t("library.coverError") }),
    });
  };

  const onSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    if (title.trim().length === 0) {
      toast.error({ title: t("validation.required") });
      return;
    }
    const year = releaseYear.trim();
    update.mutate(
      {
        id: media.id,
        title: title.trim(),
        pubStatus,
        format: format.trim() === "" ? null : format.trim(),
        synopsis: synopsis.trim() === "" ? null : synopsis.trim(),
        releaseYear: year === "" ? null : Number(year),
        genres: genres
          .split(",")
          .map((genre) => genre.trim())
          .filter(Boolean),
        unpin: release,
      },
      {
        onSuccess: () => {
          onOpenChange(false);
          toast.success({ title: t("library.editSaved") });
        },
        onError: () => toast.error({ title: t("library.editError") }),
      },
    );
  };

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent closeLabel={t("library.closeAria")}>
        <DialogTitle>{t("library.editTitle")}</DialogTitle>
        <p className="mt-1 text-sm text-text-tertiary">{t("library.editHint")}</p>

        <form onSubmit={onSubmit} className="mt-5 grid grid-cols-2 gap-4">
          <div className="col-span-2">
            <InputField
              label={t("library.fieldTitle")}
              value={title}
              onChange={(event) => setTitle(event.target.value)}
            />
          </div>

          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-text-secondary">
              {t("library.fieldStatus")}
            </span>
            <select
              value={pubStatus}
              onChange={(event) => setPubStatus(event.target.value)}
              className={SELECT_CLASSES}
            >
              {PUBLICATION_STATUS_VALUES.map((status) => (
                <option key={status} value={status}>
                  {t(`pubStatus.${status}`)}
                </option>
              ))}
            </select>
          </label>

          <InputField
            label={t("library.fieldFormat")}
            value={format}
            onChange={(event) => setFormat(event.target.value)}
          />
          <InputField
            label={t("library.fieldYear")}
            inputMode="numeric"
            value={releaseYear}
            onChange={(event) => setReleaseYear(event.target.value)}
          />
          <InputField
            label={t("library.fieldGenres")}
            value={genres}
            onChange={(event) => {
              genresTouched.current = true;
              setGenres(event.target.value);
            }}
          />
          <div className="col-span-2">
            <TextareaField
              label={t("library.fieldSynopsis")}
              rows={4}
              value={synopsis}
              onChange={(event) => setSynopsis(event.target.value)}
            />
          </div>

          <fieldset className="col-span-2 rounded-sm border border-border-subtle p-3">
            <legend className="px-1 text-xs uppercase tracking-wide text-text-tertiary">
              {t("library.coverHeading")}
            </legend>
            <InputField
              label={t("library.fieldCoverUrl")}
              value={coverUrl}
              placeholder="https://…"
              onChange={(event) => setCoverUrl(event.target.value)}
            />
            <div className="mt-3 flex items-center justify-end gap-2">
              {media.cover_asset_id ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={setCover.isPending}
                  onClick={() => applyCover(null)}
                >
                  {t("library.coverClear")}
                </Button>
              ) : null}
              <Button
                type="button"
                variant="secondary"
                size="sm"
                disabled={setCover.isPending || coverUrl.trim().length === 0}
                onClick={() => applyCover(coverUrl.trim())}
              >
                {t("library.coverSet")}
              </Button>
            </div>
            <p className="mt-2 text-xs text-text-tertiary">{t("library.coverHint")}</p>
          </fieldset>

          {overrides.length > 0 ? (
            <fieldset className="col-span-2 rounded-sm border border-border-subtle p-3">
              <legend className="px-1 text-xs uppercase tracking-wide text-text-tertiary">
                {t("library.editPinned")}
              </legend>
              <div className="flex flex-col gap-2">
                {PINNABLE.filter((entry) => overrides.includes(entry.field)).map((entry) => (
                  <label
                    key={entry.field}
                    className="flex items-center gap-2 text-sm text-text-secondary"
                  >
                    <input
                      type="checkbox"
                      checked={release.includes(entry.field)}
                      onChange={(event) => toggleRelease(entry.field, event.target.checked)}
                    />
                    {t(entry.labelKey)}
                  </label>
                ))}
              </div>
              <p className="mt-2 text-xs text-text-tertiary">{t("library.editPinnedHint")}</p>
            </fieldset>
          ) : null}

          <div className="col-span-2 mt-2 flex justify-end gap-2">
            <Button variant="secondary" type="button" onClick={() => onOpenChange(false)}>
              {t("library.cancel")}
            </Button>
            <Button type="submit" disabled={update.isPending}>
              {t("library.editSubmit")}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
