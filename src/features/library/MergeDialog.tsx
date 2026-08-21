import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  Skeleton,
  useToast,
} from "@/components/ui";
import { media_get, type MergePreview } from "@/api";
import { useMergeApply, useMergePlan } from "./merge";

/* MISSION-089 — Merge dialog. Two selected titles in, one survivor out:
   pick which title survives, preview the field conflicts and what will move
   (nodes / review / tracking / collections), then apply. The merge parks an
   undo image in the trash, so it is reversible from the Trash page. */

export interface MergeDialogProps {
  ids: [string, string];
  open: boolean;
  onClose: () => void;
  onMerged: () => void;
}

function useTitle(id: string) {
  return useQuery({
    queryKey: ["media", "detail", id],
    queryFn: () => media_get({ id }),
    enabled: id !== "",
  });
}

export function MergeDialog({ ids, open, onClose, onMerged }: MergeDialogProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const plan = useMergePlan();
  const apply = useMergeApply();

  const [survivorIndex, setSurvivorIndex] = useState<0 | 1>(0);
  const [preview, setPreview] = useState<MergePreview | null>(null);

  const first = useTitle(ids[0]);
  const second = useTitle(ids[1]);
  const titles = [first.data?.title_main, second.data?.title_main];
  const survivorId = ids[survivorIndex];
  const duplicateId = ids[survivorIndex === 0 ? 1 : 0];

  const reset = () => {
    setPreview(null);
    setSurvivorIndex(0);
  };

  const close = () => {
    reset();
    onClose();
  };

  const runPreview = () => {
    plan.mutate(
      { survivorId, duplicateId },
      {
        onSuccess: setPreview,
        onError: () => toast.error({ title: t("merge.planFailed") }),
      },
    );
  };

  const runApply = () => {
    apply.mutate(
      { survivorId, duplicateId },
      {
        onSuccess: () => {
          toast.success({ title: t("merge.appliedToast") });
          reset();
          onMerged();
        },
        onError: () => toast.error({ title: t("merge.applyFailed") }),
      },
    );
  };

  return (
    <Dialog open={open} onOpenChange={(value) => !value && close()}>
      <DialogContent closeLabel={t("merge.close")}>
        <DialogTitle>{t("merge.title")}</DialogTitle>
        <DialogDescription>{t("merge.hint")}</DialogDescription>

        <div className="mt-5 flex flex-col gap-4 text-sm">
          {!preview ? (
            <>
              <div
                role="radiogroup"
                aria-label={t("merge.pickSurvivor")}
                className="flex flex-col gap-2"
              >
                {ids.map((id, index) =>
                  titles[index] ? (
                    <label
                      key={id}
                      className="flex cursor-pointer items-center gap-2 rounded-sm border border-border-subtle p-3 text-text-primary transition-colors duration-150 ease-out hover:bg-bg-hover"
                    >
                      <input
                        type="radio"
                        name="merge-survivor"
                        checked={survivorIndex === index}
                        onChange={() => setSurvivorIndex(index as 0 | 1)}
                      />
                      {titles[index]}
                    </label>
                  ) : (
                    <Skeleton key={id} className="h-11" />
                  ),
                )}
              </div>
              <p className="text-text-tertiary">{t("merge.survivorHint")}</p>
            </>
          ) : (
            <>
              <div className="rounded-sm border border-border-subtle p-3">
                <p className="font-medium text-text-primary">
                  {t("merge.previewTitle", {
                    survivor: preview.survivor_title,
                    duplicate: preview.duplicate_title,
                  })}
                </p>
                <p className="mt-1 text-text-secondary">
                  {t("merge.movingSummary", {
                    nodes: preview.nodes_to_move,
                    collections: preview.collections_to_move,
                  })}
                </p>
                {preview.move_review || preview.move_tracking ? (
                  <p className="mt-1 text-text-secondary">
                    {[
                      preview.move_review ? t("merge.movesReview") : null,
                      preview.move_tracking ? t("merge.movesTracking") : null,
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </p>
                ) : null}
              </div>

              <div>
                <h3 className="text-xs font-medium text-text-secondary">
                  {t("merge.conflicts", { count: preview.conflicts.length })}
                </h3>
                {preview.conflicts.length === 0 ? (
                  <p className="mt-1 text-text-tertiary">{t("merge.noConflicts")}</p>
                ) : (
                  <ul className="mt-2 flex flex-col gap-1.5">
                    {preview.conflicts.map((conflict) => (
                      <li key={conflict.field} className="flex flex-wrap items-baseline gap-2">
                        <span className="w-32 shrink-0 truncate text-xs text-text-tertiary">
                          {conflict.field}
                        </span>
                        <span className="min-w-0 flex-1 truncate text-text-primary">
                          {conflict.survivor}
                        </span>
                        <span className="min-w-0 flex-1 truncate text-text-tertiary">
                          {conflict.duplicate}
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              <p className="text-text-tertiary">{t("merge.undoHint")}</p>
            </>
          )}

          <div className="mt-2 flex justify-end gap-2">
            {preview ? (
              <>
                <Button variant="secondary" onClick={() => setPreview(null)}>
                  {t("merge.back")}
                </Button>
                <DialogClose asChild>
                  <Button onClick={runApply} disabled={apply.isPending}>
                    {apply.isPending ? t("merge.applying") : t("merge.apply")}
                  </Button>
                </DialogClose>
              </>
            ) : (
              <>
                <DialogClose asChild>
                  <Button variant="secondary">{t("merge.close")}</Button>
                </DialogClose>
                <Button onClick={runPreview} disabled={plan.isPending}>
                  {plan.isPending ? t("merge.loading") : t("merge.preview")}
                </Button>
              </>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
