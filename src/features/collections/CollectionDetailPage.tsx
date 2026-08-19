import { useState } from "react";
import { ArrowDown, ArrowUp, ChevronLeft, FolderPlus, GripVertical, X } from "lucide-react";
import { Link, useParams } from "react-router";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { useToast } from "@/components/ui";
import { queryKeys } from "@/api";
import { useAssetViews } from "@/features/library/api";
import { MediaRow } from "@/features/library/MediaRow";
import { useCollectionsQuery } from "@/features/collections/api";
import { useCollectionMembersQuery, useRemoveMember, useReorderMembers } from "./api";

/* MISSION-076 — Collection detail. Ordered member list backed by native HTML5
   drag-and-drop (no external DnD dependency). Drag handlers never read
   `dataTransfer`, so jsdom tests drive them with fireEvent. Up/Down buttons
   provide the accessible, test-reliable path to the same reorder mutation. */

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

  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);

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
          <h1 className="min-w-0 truncate text-base font-semibold text-text-primary">
            {collection.name}
          </h1>
          <span className="text-sm tabular-nums text-text-secondary">
            {t("collections.memberCount", { count: members.length })}
          </span>
        </div>
      </div>

      {members.length === 0 ? (
        <div className="flex-1">
          <EmptyState
            icon={FolderPlus}
            title={t("collections.emptyDetailTitle")}
            hint={t("collections.emptyDetailHint")}
          />
        </div>
      ) : (
        <div className="flex-1 space-y-1 overflow-y-auto px-5 py-4">
          <p className="px-1 pb-2 text-xs text-text-tertiary">{t("collections.dragHint")}</p>
          {members.map((member, index) => (
            <div
              key={member.media.id}
              draggable
              onDragStart={() => setDragIndex(index)}
              onDragOver={(event) => {
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
                <span aria-hidden="true" className="shrink-0 cursor-grab text-text-tertiary">
                  <GripVertical size={16} />
                </span>
                <div className="min-w-0 flex-1">
                  <MediaRow
                    item={member.media}
                    cover={covers.data?.find((c) => c.id === member.media.cover_asset_id)}
                    dense
                  />
                </div>
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
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
