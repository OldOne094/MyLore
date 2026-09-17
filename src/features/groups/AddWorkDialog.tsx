/* MISSION-117 — Put a work on my shelf.

   The work key is asked of the domain (`reading_group_work_key`) rather than
   re-derived here: the fold + hash must have exactly one implementation, or two
   devices would silently shelve the same book twice. Entering a title that
   someone else already shelved resolves to the same key, so the entry lands on
   their row. */

import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { useTranslation } from "react-i18next";
import { Button, Dialog, DialogContent, DialogTitle, InputField } from "@/components/ui";
import { useToast } from "@/components/ui";
import { CONTENT_TYPES } from "@/features/library/contentTypes";
import { mapIssuesToKeys } from "@/features/library/AddMediaSchema";
import { resolveWorkKey, useSetShelfEntry } from "@/features/groups/api";
import {
  addWorkSchema,
  SHELF_STATUS_VALUES,
  type AddWorkFormInput,
  type AddWorkFormValues,
} from "@/features/groups/AddWorkSchema";

const SELECT_CLASSES =
  "h-[var(--control-height)] w-full rounded-sm border border-border-strong bg-bg-surface px-2 text-base text-text-primary";

export interface AddWorkDialogProps {
  groupId: string;
  memberId: string;
  open: boolean;
  /** Pre-fills the title when the dialog is opened for an existing work. */
  defaultTitle?: string;
  onOpenChange: (open: boolean) => void;
}

export function AddWorkDialog({
  groupId,
  memberId,
  open,
  defaultTitle = "",
  onOpenChange,
}: AddWorkDialogProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const setShelf = useSetShelfEntry(groupId);

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<AddWorkFormInput, unknown, AddWorkFormValues>({
    resolver: zodResolver(addWorkSchema, { error: mapIssuesToKeys }),
    mode: "onBlur",
    defaultValues: {
      title: defaultTitle,
      author: "",
      year: "",
      contentType: "book",
      status: "in_progress",
      progress: 0,
    },
  });

  const onSubmit = handleSubmit(async (values) => {
    const workKey = await resolveWorkKey({
      title: values.title,
      author: values.author ?? null,
      year: values.year ?? null,
    });
    setShelf.mutate(
      {
        memberId,
        workKey,
        title: values.title,
        contentType: values.contentType,
        status: values.status,
        progress: values.progress,
      },
      {
        onSuccess: () => {
          reset();
          onOpenChange(false);
          toast.success({ title: t("groupsPage.addWorkToast", { title: values.title }) });
        },
        onError: () => toast.error({ title: t("groupsPage.addWorkErrorToast") }),
      },
    );
  });

  const fieldError = (key: keyof typeof errors) =>
    errors[key]?.message ? t(errors[key]!.message as string) : undefined;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogTitle>{t("groupsPage.addWorkTitle")}</DialogTitle>
        <form className="mt-4 flex flex-col gap-4" onSubmit={onSubmit}>
          <InputField
            label={t("groupsPage.workTitleLabel")}
            autoFocus
            error={fieldError("title")}
            {...register("title")}
          />
          <div className="flex gap-3">
            <div className="min-w-0 flex-1">
              <InputField label={t("groupsPage.authorLabel")} {...register("author")} />
            </div>
            <div className="w-28">
              <InputField
                label={t("groupsPage.yearLabel")}
                inputMode="numeric"
                {...register("year")}
              />
            </div>
          </div>
          <div className="flex gap-3">
            <label className="flex min-w-0 flex-1 flex-col gap-1 text-sm text-text-secondary">
              {t("groupsPage.contentTypeLabel")}
              <select className={SELECT_CLASSES} {...register("contentType")}>
                {CONTENT_TYPES.map((type) => (
                  <option key={type} value={type}>
                    {t(`contentType.${type}`)}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex min-w-0 flex-1 flex-col gap-1 text-sm text-text-secondary">
              {t("groupsPage.statusLabel")}
              <select className={SELECT_CLASSES} {...register("status")}>
                {SHELF_STATUS_VALUES.map((status) => (
                  <option key={status} value={status}>
                    {t(`coreStatus.${status}`)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="w-32">
            <InputField
              label={t("groupsPage.progressLabel")}
              inputMode="numeric"
              error={fieldError("progress")}
              {...register("progress")}
            />
          </div>
          <p className="text-sm text-text-tertiary">{t("groupsPage.progressHint")}</p>
          <div className="flex justify-end gap-2">
            <Button variant="secondary" type="button" onClick={() => onOpenChange(false)}>
              {t("groupsPage.cancel")}
            </Button>
            <Button type="submit" disabled={isSubmitting || setShelf.isPending}>
              {t("groupsPage.addWork")}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
