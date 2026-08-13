import { useState } from "react";
import { RotateCcw, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  EmptyState,
  Skeleton,
} from "@/components/ui";
import { useToast } from "@/components/ui";
import { usePurgeTrashItem, useRestoreTrashItem, useTrashListQuery } from "./api";

/* MISSION-044 — Trash page. Soft-deleted titles live here until restored or
   purged. Restore re-creates the aggregate from its before-image; purge forgets
   it forever (destructive, guarded by a confirm dialog). */

function TrashSkeleton() {
  return (
    <div role="status" aria-label="Loading trash" className="px-6 pt-6">
      {Array.from({ length: 4 }, (_, index) => (
        <div key={index} className="mb-2 flex items-center gap-3 rounded-md px-3 py-2">
          <Skeleton className="size-8" />
          <Skeleton className="h-4 flex-1" />
          <Skeleton className="h-8 w-40" />
        </div>
      ))}
    </div>
  );
}

export function TrashPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const { data, isLoading, isError, refetch } = useTrashListQuery();
  const restore = useRestoreTrashItem();
  const purge = usePurgeTrashItem();
  const [pendingPurge, setPendingPurge] = useState<string | null>(null);

  if (isLoading) return <TrashSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={Trash2}
        title={t("trash.errorTitle")}
        hint={t("trash.errorHint")}
        action={
          <Button variant="secondary" onClick={() => void refetch()}>
            {t("trash.retry")}
          </Button>
        }
      />
    );
  }

  const items = data ?? [];
  if (items.length === 0) {
    return <EmptyState icon={Trash2} title={t("trash.emptyTitle")} hint={t("trash.emptyHint")} />;
  }

  const onRestore = (id: string, title: string) => {
    restore.mutate(id, {
      onSuccess: () => toast.success({ title: t("trash.restoredToast", { title }) }),
      onError: () => toast.error({ title: t("trash.restoreErrorToast") }),
    });
  };

  const onPurge = (id: string) => {
    setPendingPurge(null);
    const item = items.find((entry) => entry.id === id);
    purge.mutate(id, {
      onSuccess: () =>
        toast.success({ title: t("trash.purgedToast", { title: item?.title ?? "" }) }),
      onError: () => toast.error({ title: t("trash.purgeErrorToast") }),
    });
  };

  return (
    <section aria-label={t("nav.trash")} className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b border-border-subtle px-6 py-3 text-sm text-text-secondary">
        {t("trash.count", { count: items.length })}
      </div>
      <div className="flex-1 space-y-2 overflow-y-auto px-6 py-5">
        {items.map((item) => (
          <div
            key={item.id}
            className="flex items-center gap-3 rounded-md border border-border-subtle bg-bg-surface px-3 py-2"
          >
            <Trash2 size={16} aria-hidden="true" className="shrink-0 text-text-tertiary" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-text-primary">{item.title}</p>
              <p className="text-xs text-text-tertiary">{item.deleted_at}</p>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={() => onRestore(item.id, item.title)}
                disabled={restore.isPending || purge.isPending}
              >
                <RotateCcw size={14} aria-hidden="true" />
                {t("trash.restore")}
              </Button>
              <Button
                variant="danger"
                size="sm"
                onClick={() => setPendingPurge(item.id)}
                disabled={restore.isPending || purge.isPending}
              >
                <Trash2 size={14} aria-hidden="true" />
                {t("trash.purge")}
              </Button>
            </div>
          </div>
        ))}
      </div>

      <Dialog open={pendingPurge !== null} onOpenChange={(open) => !open && setPendingPurge(null)}>
        <DialogContent>
          <DialogTitle>{t("trash.purgeDialogTitle")}</DialogTitle>
          <DialogDescription>{t("trash.purgeDialogHint")}</DialogDescription>
          <div className="mt-6 flex justify-end gap-2">
            <Button variant="secondary" onClick={() => setPendingPurge(null)}>
              {t("trash.cancel")}
            </Button>
            <Button
              variant="danger"
              onClick={() => pendingPurge && onPurge(pendingPurge)}
              aria-label={t("trash.purgeForeverAria")}
            >
              {t("trash.purgeForever")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </section>
  );
}
