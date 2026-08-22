import { useState } from "react";
import { Download, FolderPlus, ListCheck, ListPlus, Tag, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
  InputField,
  Popover,
  PopoverClose,
  PopoverContent,
  PopoverTrigger,
  useToast,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import { useRestoreTrashItem } from "@/features/trash/api";
import {
  useBulkAddTag,
  useBulkAddToCollection,
  useBulkDelete,
  useBulkSetStatus,
  useCollectionListQuery,
} from "./bulk";
import { GitMerge } from "lucide-react";
import { MergeDialog } from "./MergeDialog";
import type { LibraryFilters } from "./filters";

/* MISSION-045 — Library action bar. Appears in bulk-select mode with one or
   more titles selected. Actions: set tracking status (status engine applies
   server-side), add a personal tag, add to a collection, soft-delete to trash
   (undo restores the whole batch), and a placeholder Export (arrives later).
   MISSION-078 — with active filters the bar can switch its scope from the
   selected titles to the whole filtered selection (resolved server-side), and
   every action surfaces a per-item change summary. */

/** Order matches CoreStatus::ALL in the Rust domain. Module-local: nothing imports it. */
const CORE_STATUSES = [
  "planned",
  "in_progress",
  "completed",
  "on_hold",
  "dropped",
  "repeat",
  "wishlist",
] as const;

export interface BulkActionBarProps {
  ids: string[];
  /** Active library filters; when set, the bar offers a "whole filtered
      selection" scope (MISSION-078). */
  filter?: LibraryFilters | null;
  /** Total titles matching the active filters (drives the scope label). */
  matchingCount?: number;
  /** Called after a successful action so the page can leave select mode. */
  onDone: () => void;
}

export function BulkActionBar({ ids, filter, matchingCount, onDone }: BulkActionBarProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const [tagOpen, setTagOpen] = useState(false);
  const [tag, setTag] = useState("");
  const [scope, setScope] = useState<"selected" | "filtered">("selected");
  const [mergeOpen, setMergeOpen] = useState(false);

  const setStatus = useBulkSetStatus();
  const addTag = useBulkAddTag();
  const bulkDelete = useBulkDelete();
  const addToCollection = useBulkAddToCollection();
  const restoreTrash = useRestoreTrashItem();
  const collections = useCollectionListQuery();

  const busy =
    setStatus.isPending || addTag.isPending || bulkDelete.isPending || addToCollection.isPending;

  // The filtered scope only makes sense when there are active filters and the
  // selection is smaller than the matching set. If filters changed under us,
  // fall back to the selected scope rather than surprising the user.
  const canScope = Boolean(filter) && (matchingCount ?? 0) > ids.length;
  const effectiveScope = scope === "filtered" && canScope ? "filtered" : "selected";
  const opFilter = effectiveScope === "filtered" ? filter : null;

  const reportPartial = (failed: number) => {
    if (failed > 0) {
      toast.error({ title: t("bulk.partialFailures", { count: failed }) });
    }
  };

  const handleStatus = (status: string) => {
    setStatus.mutate(
      { ids, core_status: status, filter: opFilter },
      {
        onSuccess: (result) => {
          toast.success({
            title: t("bulk.statusSetSummary", {
              succeeded: result.succeeded,
              total: result.total,
            }),
          });
          reportPartial(result.failed);
          onDone();
        },
        onError: () => toast.error({ title: t("bulk.statusError") }),
      },
    );
  };

  const handleTagSubmit = () => {
    const trimmed = tag.trim();
    if (!trimmed) return;
    addTag.mutate(
      { ids, tag: trimmed, filter: opFilter },
      {
        onSuccess: (result) => {
          toast.success({ title: t("bulk.tagAddedSummary", { count: result.succeeded }) });
          reportPartial(result.failed);
          setTag("");
          setTagOpen(false);
          onDone();
        },
        onError: () => toast.error({ title: t("bulk.tagError") }),
      },
    );
  };

  const handleAddToList = (collectionId: string) => {
    const name = collections.data?.find((c) => c.id === collectionId)?.name ?? "";
    addToCollection.mutate(
      { collection_id: collectionId, ids, filter: opFilter },
      {
        onSuccess: (result) => {
          toast.success({ title: t("bulk.listAdded", { name }) });
          reportPartial(result.failed);
          onDone();
        },
        onError: () => toast.error({ title: t("bulk.listError") }),
      },
    );
  };

  const handleDelete = () => {
    bulkDelete.mutate(
      { ids, filter: opFilter },
      {
        onSuccess: (result) => {
          toast.success({
            title: t("trash.deletedToast", { count: result.summary.succeeded }),
            action: {
              label: t("trash.undo"),
              onClick: () => {
                void Promise.all(result.trash_ids.map((id) => restoreTrash.mutateAsync(id))).then(
                  () =>
                    toast.success({
                      title: t("bulk.restoredToast", { count: result.trash_ids.length }),
                    }),
                  () => toast.error({ title: t("trash.restoreErrorToast") }),
                );
              },
            },
          });
          reportPartial(result.summary.failed);
          onDone();
        },
        onError: () => toast.error({ title: t("bulk.deleteError") }),
      },
    );
  };

  return (
    <div
      role="toolbar"
      aria-label={t("library.select")}
      className="flex shrink-0 items-center gap-2 border-t border-border-subtle bg-bg-surface px-5 py-3"
    >
      <span className="text-sm tabular-nums text-text-secondary">
        {t("library.selectionCount", { count: ids.length })}
      </span>

      {canScope && (
        <div
          role="group"
          aria-label={t("bulk.scope")}
          className="inline-flex items-center gap-1 rounded-full border border-border-subtle bg-bg-surface p-1"
        >
          {(["selected", "filtered"] as const).map((value) => {
            const active = effectiveScope === value;
            const label =
              value === "selected"
                ? t("bulk.scopeSelected", { count: ids.length })
                : t("bulk.scopeFiltered", { count: matchingCount });
            return (
              <button
                key={value}
                type="button"
                aria-pressed={active}
                disabled={busy}
                onClick={() => setScope(value)}
                className={cn(
                  "rounded-full border-none bg-transparent px-2.5 py-1 text-sm text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-50",
                  active && "bg-accent text-bg-surface hover:bg-accent",
                )}
              >
                {label}
              </button>
            );
          })}
        </div>
      )}

      <div className="ms-auto flex items-center gap-2">
        <Popover>
          <PopoverTrigger asChild>
            <Button
              variant="secondary"
              size="sm"
              disabled={busy}
              aria-label={t("bulk.status")}
              className="h-[var(--control-height-compact)] px-3 text-sm"
            >
              <ListCheck size={14} aria-hidden="true" />
              {t("bulk.status")}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-auto">
            <div className="flex w-44 flex-col gap-1">
              {CORE_STATUSES.map((status) => (
                <PopoverClose key={status} asChild>
                  <button
                    type="button"
                    onClick={() => handleStatus(status)}
                    className="flex items-center rounded-sm px-2 py-1.5 text-sm transition-colors duration-150 ease-out hover:bg-bg-hover"
                  >
                    {t(`coreStatus.${status}`)}
                  </button>
                </PopoverClose>
              ))}
            </div>
          </PopoverContent>
        </Popover>

        <Dialog open={tagOpen} onOpenChange={setTagOpen}>
          <DialogTrigger asChild>
            <Button
              variant="secondary"
              size="sm"
              disabled={busy}
              aria-label={t("bulk.tag")}
              className="h-[var(--control-height-compact)] px-3 text-sm"
            >
              <Tag size={14} aria-hidden="true" />
              {t("bulk.tag")}
            </Button>
          </DialogTrigger>
          <DialogContent closeLabel={t("a11y.close")}>
            <DialogTitle>{t("bulk.tagDialogTitle")}</DialogTitle>
            <DialogDescription>{t("bulk.tagDialogHint")}</DialogDescription>
            <form
              onSubmit={(event) => {
                event.preventDefault();
                handleTagSubmit();
              }}
              className="mt-4 flex flex-col gap-4"
            >
              <InputField
                label={t("bulk.fieldTag")}
                placeholder={t("bulk.tagPlaceholder")}
                value={tag}
                onChange={(event) => setTag(event.target.value)}
              />
              <div className="flex justify-end gap-2">
                <DialogClose asChild>
                  <Button variant="ghost" size="sm" onClick={() => setTag("")}>
                    {t("library.cancel")}
                  </Button>
                </DialogClose>
                <Button type="submit" size="sm" disabled={!tag.trim() || addTag.isPending}>
                  {t("bulk.tagSubmit")}
                </Button>
              </div>
            </form>
          </DialogContent>
        </Dialog>

        <Popover>
          <PopoverTrigger asChild>
            <Button
              variant="secondary"
              size="sm"
              disabled={busy}
              aria-label={t("bulk.list")}
              className="h-[var(--control-height-compact)] px-3 text-sm"
            >
              <ListPlus size={14} aria-hidden="true" />
              {t("bulk.list")}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-auto">
            {collections.data && collections.data.length > 0 ? (
              <div className="flex w-52 flex-col gap-1">
                {collections.data.map((collection) => (
                  <PopoverClose key={collection.id} asChild>
                    <button
                      type="button"
                      onClick={() => handleAddToList(collection.id)}
                      className="flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm transition-colors duration-150 ease-out hover:bg-bg-hover"
                    >
                      <FolderPlus size={14} aria-hidden="true" className="text-text-secondary" />
                      <span className="truncate">{collection.name}</span>
                    </button>
                  </PopoverClose>
                ))}
              </div>
            ) : (
              <p className="w-56 text-sm text-text-secondary">{t("bulk.listEmpty")}</p>
            )}
          </PopoverContent>
        </Popover>

        <Button
          variant="danger"
          size="sm"
          disabled={busy}
          onClick={handleDelete}
          aria-label={t("bulk.delete")}
          className="h-[var(--control-height-compact)] px-3 text-sm"
        >
          <Trash2 size={14} aria-hidden="true" />
          {t("bulk.delete")}
        </Button>

        {ids.length === 2 && (
          <>
            <Button
              variant="secondary"
              size="sm"
              disabled={busy}
              onClick={() => setMergeOpen(true)}
              aria-label={t("merge.action")}
              className="h-[var(--control-height-compact)] px-3 text-sm"
            >
              <GitMerge size={14} aria-hidden="true" />
              {t("merge.action")}
            </Button>
            <MergeDialog
              ids={[ids[0], ids[1]]}
              open={mergeOpen}
              onClose={() => setMergeOpen(false)}
              onMerged={onDone}
            />
          </>
        )}

        <span title={t("bulk.exportSoon")}>
          <Button
            variant="ghost"
            size="sm"
            disabled
            aria-label={t("bulk.export")}
            className="h-[var(--control-height-compact)] px-3 text-sm"
          >
            <Download size={14} aria-hidden="true" />
            {t("bulk.export")}
          </Button>
        </span>
      </div>
    </div>
  );
}
