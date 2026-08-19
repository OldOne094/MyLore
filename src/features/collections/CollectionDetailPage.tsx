import { useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  ChevronLeft,
  FolderPlus,
  GripVertical,
  Sparkles,
  Wand2,
  X,
} from "lucide-react";
import { Link, useParams } from "react-router";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogTitle,
  EmptyState,
  Skeleton,
} from "@/components/ui";
import { useToast } from "@/components/ui";
import { queryKeys } from "@/api";
import type { SmartFilter } from "@/api";
import { useAssetViews, useMediaFacetsQuery } from "@/features/library/api";
import { MediaRow } from "@/features/library/MediaRow";
import { useCollectionsQuery } from "@/features/collections/api";
import {
  useCollectionMembersQuery,
  useRemoveMember,
  useReorderMembers,
  useUpdateSmartFilter,
} from "./api";
import { SmartFilterForm } from "./SmartFilterForm";
import { EMPTY_SMART_FILTER } from "./smartFilter";

/* MISSION-076/077 — Collection detail. Ordered member list backed by native
   HTML5 drag-and-drop (no external DnD dependency). Drag handlers never read
   `dataTransfer`, so jsdom tests drive them with fireEvent. Up/Down buttons
   provide the accessible, test-reliable path to the same reorder mutation.
   MISSION-077: smart collections render their computed members read-only —
   no reorder/remove controls, an "Edit filter" dialog instead. */

function moveItem<T>(list: T[], from: number, to: number): T[] {
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

function MembersSkeleton() {
  return (
    <div role="status" aria-label="Loading members" className="px-6 pt-6">
      {Array.from({ length: 4 }, (_, index) => (
        <div key={index} className="mb-2 flex items-center gap-3 rounded-md px-3 py-2">
          <Skeleton className="size-8" />
          <Skeleton className="h-4 flex-1" />
          <Skeleton className="size-8" />
        </div>
      ))}
    </div>
  );
}

export function CollectionDetailPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const { collectionId = "" } = useParams();
  const queryClient = useQueryClient();

  const collections = useCollectionsQuery();
  const membersQuery = useCollectionMembersQuery(collectionId);
  const removeMember = useRemoveMember();
  const reorder = useReorderMembers();
  const updateSmart = useUpdateSmartFilter();
  const { data: facets } = useMediaFacetsQuery();

  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [editFilter, setEditFilter] = useState<SmartFilter>(EMPTY_SMART_FILTER);

  const collection = collections.data?.find((c) => c.id === collectionId);
  const members = membersQuery.data ?? [];
  const covers = useAssetViews(members.map((m) => m.media.cover_asset_id ?? ""));
  const loading = collections.isLoading || (membersQuery.isLoading && !membersQuery.isError);

  if (loading) return <MembersSkeleton />;
  if (collections.isError) {
    return (
      <EmptyState
        icon={FolderPlus}
        title={t("collections.errorTitle")}
        hint={t("collections.errorHint")}
      />
    );
  }

  if (!collection) {
    return <EmptyState icon={FolderPlus} title={t("collections.notFoundTitle")} />;
  }

  const smart = collection.is_smart;

  if (membersQuery.isError) {
    return (
      <EmptyState
        icon={FolderPlus}
        title={t("collections.errorTitle")}
        hint={t("collections.errorHint")}
      />
    );
  }

  const memberKey = queryKeys.collection.members(collectionId);

  const resetDrag = () => {
    setDragIndex(null);
    setOverIndex(null);
  };

  const commitOrder = (orderedIds: string[], previous: typeof members) => {
    reorder.mutate(
      { collection_id: collectionId, media_ids: orderedIds },
      {
        onSuccess: () => {
          toast.success({ title: t("collections.reorderToast") });
          resetDrag();
        },
        onError: () => {
          queryClient.setQueryData(memberKey, previous);
          toast.error({ title: t("collections.reorderError") });
          resetDrag();
        },
      },
    );
  };

  const handleDrop = () => {
    if (dragIndex === null || overIndex === null || dragIndex === overIndex) {
      resetDrag();
      return;
    }
    const previous = members;
    const ordered = moveItem(previous, dragIndex, overIndex);
    queryClient.setQueryData(memberKey, ordered);
    commitOrder(
      ordered.map((m) => m.media.id),
      previous,
    );
  };

  const moveBy = (index: number, delta: -1 | 1) => {
    const to = index + delta;
    if (to < 0 || to >= members.length) return;
    const previous = members;
    const ordered = moveItem(previous, index, to);
    queryClient.setQueryData(memberKey, ordered);
    commitOrder(
      ordered.map((m) => m.media.id),
      previous,
    );
  };

  const remove = (mediaId: string, title: string) => {
    const previous = members;
    queryClient.setQueryData(
      memberKey,
      previous.filter((m) => m.media.id !== mediaId),
    );
    removeMember.mutate(
      { collection_id: collectionId, media_id: mediaId },
      {
        onSuccess: () => {
          toast.success({ title: t("collections.removeMemberToast", { title }) });
        },
        onError: () => {
          queryClient.setQueryData(memberKey, previous);
          toast.error({ title: t("collections.removeMemberError") });
        },
      },
    );
  };

  return (
    <section aria-label={t("nav.collections")} className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b border-border-subtle px-5 py-3">
        <Link
          to="/collections"
          aria-label={t("collections.backAria")}
          className="mb-1 inline-flex items-center gap-1 text-xs text-text-secondary transition-colors duration-150 ease-out hover:text-text-primary"
        >
          <ChevronLeft size={14} aria-hidden="true" className="rtl:rotate-180" />
          {t("collections.backToCollections")}
        </Link>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <h1 className="min-w-0 truncate text-base font-semibold text-text-primary">
              {collection.name}
            </h1>
            {smart && (
              <span
                className="inline-flex shrink-0 items-center gap-1 rounded-full border border-accent/30 bg-accent/10 px-2 py-0.5 text-[11px] font-medium text-accent"
                aria-label={t("collections.smartBadge")}
              >
                <Sparkles size={11} aria-hidden="true" />
                {t("collections.smart")}
              </span>
            )}
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {smart ? (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => {
                  setEditFilter(collection.filter ?? EMPTY_SMART_FILTER);
                  setEditOpen(true);
                }}
              >
                <Wand2 size={14} aria-hidden="true" />
                {t("collections.editFilter")}
              </Button>
            ) : null}
            <span className="text-sm tabular-nums text-text-secondary">
              {t("collections.memberCount", { count: members.length })}
            </span>
          </div>
        </div>
      </div>

      {members.length === 0 ? (
        <div className="flex-1">
          {smart ? (
            <EmptyState
              icon={Sparkles}
              title={t("collections.smartEmptyTitle")}
              hint={t("collections.smartEmptyHint")}
            />
          ) : (
            <EmptyState
              icon={FolderPlus}
              title={t("collections.emptyDetailTitle")}
              hint={t("collections.emptyDetailHint")}
            />
          )}
        </div>
      ) : (
        <div className="flex-1 space-y-1 overflow-y-auto px-5 py-4">
          {smart ? (
            <p className="px-1 pb-2 text-xs text-text-tertiary">{t("collections.computedNote")}</p>
          ) : (
            <p className="px-1 pb-2 text-xs text-text-tertiary">{t("collections.dragHint")}</p>
          )}
          {members.map((member, index) => (
            <div
              key={member.media.id}
              draggable={!smart}
              onDragStart={() => setDragIndex(index)}
              onDragOver={(event) => {
                if (smart) return;
                event.preventDefault();
                if (overIndex !== index) setOverIndex(index);
              }}
              onDrop={() => handleDrop()}
              onDragEnd={resetDrag}
              className={
                overIndex === index && dragIndex !== null && dragIndex !== index
                  ? "rounded-md ring-1 ring-accent"
                  : "rounded-md"
              }
            >
              <div className="flex items-center gap-1">
                {!smart && (
                  <span aria-hidden="true" className="shrink-0 cursor-grab text-text-tertiary">
                    <GripVertical size={16} />
                  </span>
                )}
                <div className="min-w-0 flex-1">
                  <MediaRow
                    item={member.media}
                    cover={covers.data?.find((c) => c.id === member.media.cover_asset_id)}
                    dense
                  />
                </div>
                {!smart && (
                  <div className="flex shrink-0 items-center gap-0.5">
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={t("collections.moveUp")}
                      disabled={index === 0}
                      onClick={() => moveBy(index, -1)}
                      className="px-1.5"
                    >
                      <ArrowUp size={14} aria-hidden="true" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={t("collections.moveDown")}
                      disabled={index === members.length - 1}
                      onClick={() => moveBy(index, 1)}
                      className="px-1.5"
                    >
                      <ArrowDown size={14} aria-hidden="true" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={t("collections.remove")}
                      onClick={() => remove(member.media.id, member.media.title)}
                      className="px-1.5 text-text-secondary hover:text-text-danger"
                    >
                      <X size={14} aria-hidden="true" />
                    </Button>
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      <Dialog open={editOpen} onOpenChange={setEditOpen}>
        <DialogContent closeLabel={t("a11y.close")} className="w-auto">
          <DialogTitle>{t("collections.editFilterDialogTitle")}</DialogTitle>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              updateSmart.mutate(
                { collection_id: collectionId, filter: editFilter },
                {
                  onSuccess: () => {
                    setEditOpen(false);
                    toast.success({ title: t("collections.updatedFilterToast") });
                  },
                  onError: () => toast.error({ title: t("collections.updateFilterError") }),
                },
              );
            }}
            className="mt-4 flex flex-col gap-4"
          >
            <SmartFilterForm value={editFilter} onChange={setEditFilter} facets={facets} />
            <div className="flex justify-end gap-2">
              <DialogClose asChild>
                <Button variant="ghost" size="sm" onClick={() => setEditOpen(false)}>
                  {t("collections.cancel")}
                </Button>
              </DialogClose>
              <Button type="submit" size="sm" disabled={updateSmart.isPending}>
                {t("collections.saveAsCollectionSubmit")}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </section>
  );
}
