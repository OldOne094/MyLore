import { zodResolver } from "@hookform/resolvers/zod";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { useToast } from "@/components/ui";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui";
import { DialogTrigger } from "@/components/ui";
import { InputField, TextareaField } from "@/components/ui";
import { useAddMedia } from "./api";
import {
  addMediaSchema,
  CONTENT_TYPE_VALUES,
  mapIssuesToKeys,
  PUBLICATION_STATUS_VALUES,
  type AddMediaFormInput,
  type AddMediaFormValues,
} from "./AddMediaSchema";

/* MISSION-038 — Add-a-title dialog. React Hook Form + Zod; schema messages are
   i18n keys (mapIssuesToKeys) so field errors render translated. Numeric and
   status fields arrive as free text and are normalized in the schema. */

const SELECT_CLASSES =
  "h-[var(--control-height)] w-full rounded-sm border bg-bg-base px-3 text-base text-text-primary " +
  "transition-colors duration-150 ease-out hover:border-accent focus-visible:outline-none";

export interface AddMediaDialogProps {
  trigger: React.ReactNode;
}

export function AddMediaDialog({ trigger }: AddMediaDialogProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const addMedia = useAddMedia();
  const [open, setOpen] = useState(false);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<AddMediaFormInput, unknown, AddMediaFormValues>({
    resolver: zodResolver(addMediaSchema, { error: mapIssuesToKeys }),
    mode: "onBlur",
    defaultValues: {
      title: "",
      contentType: "other",
      format: "",
      pubStatus: "",
      synopsis: "",
      releaseYear: "",
      language: "",
      country: "",
      pages: "",
      durationMin: "",
      epCount: "",
      chCount: "",
      genres: "",
    },
  });

  const onSubmit = handleSubmit((values) => {
    if (isSubmitting) return;
    addMedia.mutate(values, {
      onSuccess: () => {
        setOpen(false);
        toast.success({ title: t("library.addedSuccess") });
      },
      onError: () => {
        toast.error({ title: t("library.addedError") });
      },
    });
  });

  const fieldError = (key: keyof typeof errors) =>
    errors[key]?.message ? t(errors[key]!.message as string) : undefined;

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent closeLabel={t("library.closeAria")}>
        <DialogTitle>{t("library.dialogTitle")}</DialogTitle>
        <DialogDescription>{t("library.dialogHint")}</DialogDescription>
        <form onSubmit={onSubmit} className="mt-5 grid grid-cols-2 gap-4">
          <div className="col-span-2">
            <InputField
              label={t("library.fieldTitle")}
              placeholder={t("library.titlePlaceholder")}
              error={fieldError("title")}
              autoFocus
              {...register("title")}
            />
          </div>

          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-text-secondary">
              {t("library.fieldType")}
            </span>
            <select {...register("contentType")} className={SELECT_CLASSES}>
              {CONTENT_TYPE_VALUES.map((type) => (
                <option key={type} value={type}>
                  {t(`contentType.${type}`)}
                </option>
              ))}
            </select>
          </label>

          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-text-secondary">
              {t("library.fieldStatus")}
            </span>
            <select {...register("pubStatus")} className={SELECT_CLASSES}>
              <option value="">{`${t("library.fieldStatus")} ${t("library.optional")}`}</option>
              {PUBLICATION_STATUS_VALUES.map((status) => (
                <option key={status} value={status}>
                  {t(`pubStatus.${status}`)}
                </option>
              ))}
            </select>
          </label>

          <div className="col-span-2">
            <InputField
              label={t("library.fieldFormat")}
              placeholder={t("library.formatPlaceholder")}
              error={fieldError("format")}
              {...register("format")}
            />
          </div>

          <InputField
            label={t("library.fieldYear")}
            placeholder="1999"
            error={fieldError("releaseYear")}
            {...register("releaseYear")}
          />
          <InputField
            label={t("library.fieldLanguage")}
            placeholder={t("library.languagePlaceholder")}
            error={fieldError("language")}
            {...register("language")}
          />
          <InputField
            label={t("library.fieldCountry")}
            placeholder={t("library.countryPlaceholder")}
            error={fieldError("country")}
            {...register("country")}
          />

          <InputField
            label={t("library.fieldDuration")}
            error={fieldError("durationMin")}
            {...register("durationMin")}
          />
          <InputField
            label={t("library.fieldEpisodes")}
            error={fieldError("epCount")}
            {...register("epCount")}
          />
          <InputField
            label={t("library.fieldChapters")}
            error={fieldError("chCount")}
            {...register("chCount")}
          />
          <InputField
            label={t("library.fieldPages")}
            error={fieldError("pages")}
            {...register("pages")}
          />

          <div className="col-span-2">
            <TextareaField
              label={t("library.fieldSynopsis")}
              placeholder={t("library.synopsisPlaceholder")}
              error={fieldError("synopsis")}
              {...register("synopsis")}
            />
          </div>

          <div className="col-span-2">
            <InputField
              label={t("library.fieldGenres")}
              placeholder="sci-fi, thriller"
              error={fieldError("genres")}
              {...register("genres")}
            />
            <p className="mt-1 text-xs text-text-tertiary">{t("library.genresHint")}</p>
          </div>

          <div className="col-span-2 mt-2 flex justify-end gap-2">
            <DialogClose asChild>
              <Button variant="secondary">{t("library.cancel")}</Button>
            </DialogClose>
            <Button type="submit" disabled={isSubmitting}>
              {t("library.submit")}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
