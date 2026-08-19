import { useState } from "react";
import { Folder, FolderPlus, Pencil, Plus, Trash2 } from "lucide-react";
import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
  EmptyState,
  InputField,
  Skeleton,
} from "@/components/ui";
import { useToast } from "@/components/ui";
import {
  useCollectionsQuery,
  useCreateCollection,
  useDeleteCollection,
  useRenameCollection,
} from "./api";

/* MISSION-076 — Collections page. Manual collections in a card grid: create,
   rename and delete from here; open a collection to manage its members. */

function CollectionsSkeleton() {
  return (
    <div role="status" aria-label="Loading collections" className="px-6 pt-6">
      <div className="grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-4">
        {Array.from({ length: 6 }, (_, index) => (
          <div key={index} className="rounded-xl border border-border-subtle p-4">
            <Skeleton className="mb-3 size-8" />
            <Skeleton className="mb-2 h-4 w-3/4" />
            <Skeleton className="h-3 w-1/2" />
          </div>
        ))}
      </div>
    </div>
  );
}

export function CollectionsPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const { data, isLoading, isError, refetch } = useCollectionsQuery();
  const create = useCreateCollection();
  const rename = useRenameCollection();
  const remove = useDeleteCollection();

  const [createOpen, setCreateOpen] = useState(false);
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [deleting, setDeleting] = useState<string | null>(null);

  if (isLoading) return <CollectionsSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={Folder}
        title={t("collections.errorTitle")}
        hint={t("collections.errorHint")}
        action={
          <Button variant="secondary" onClick={() => void refetch()}>
            {t("collections.retry")}
          </Button>
        }
      />
    );
  }

  const collections = data ?? [];
  if (collections.length === 0) {
    return (
      <EmptyState
        icon={Folder}
        title={t("collections.emptyTitle")}
        hint={t("collections.emptyHint")}
        action={
          <Dialog open={createOpen} onOpenChange={setCreateOpen}>
            <DialogTrigger asChild>
              <Button>
                <Plus size={16} aria-hidden="true" />
                {t("collections.create")}
              </Button>
            </DialogTrigger>
            <CreateDialog
              value={name}
              onChange={setName}
              pending={create.isPending}
              onSubmit={(value) => {
                create.mutate(value, {
                  onSuccess: (view) => {
                    setCreateOpen(false);
                    setName("");
                    toast.success({ title: t("collections.createdToast", { name: view.name }) });
                  },
                  onError: () => toast.error({ title: t("collections.createError") }),
                });
              }}
            />
          </Dialog>
        }
      />
    );
  }

  const openRename = (id: string, current: string) => {
    setRenameValue(current);
    setRenaming({ id, name: current });
  };

  return (
    <section aria-label={t("nav.collections")} className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-border-subtle px-5 py-3">
        <span className="text-sm tabular-nums text-text-secondary">
          {t("collections.memberCount", { count: collections.length })}
        </span>
        <Dialog open={createOpen} onOpenChange={setCreateOpen}>
          <DialogTrigger asChild>
            <Button size="sm">
              <Plus size={14} aria-hidden="true" />
              {t("collections.create")}
            </Button>
          </DialogTrigger>
          <CreateDialog
            value={name}
            onChange={setName}
            pending={create.isPending}
            onSubmit={(value) => {
              create.mutate(value, {
                onSuccess: (view) => {
                  setCreateOpen(false);
                  setName("");
                  toast.success({ title: t("collections.createdToast", { name: view.name }) });
                },
                onError: () => toast.error({ title: t("collections.createError") }),
              });
            }}
          />
        </Dialog>
      </div>

      <div className="flex-1 overflow-y-auto px-6 py-5">
        <div className="grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-4">
          {collections.map((collection) => (
            <div
              key={collection.id}
              className="group flex flex-col rounded-xl border border-border-subtle bg-bg-surface p-4 transition-colors duration-150 ease-out hover:border-border-strong"
            >
              <Link
                to={`/collections/${collection.id}`}
                aria-label={t("collections.open")}
                className="flex min-w-0 flex-1 flex-col"
              >
                <span className="mb-3 flex size-8 items-center justify-center rounded-md bg-bg-hover text-text-secondary">
                  <FolderPlus size={16} aria-hidden="true" />
                </span>
                <h2 className="min-w-0 truncate text-sm font-medium text-text-primary">
                  {collection.name}
                </h2>
                <p className="mt-1 text-xs tabular-nums text-text-tertiary">
                  {t("collections.memberCount", { count: collection.member_count })}
                </p>
              </Link>
              <div className="mt-4 flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  aria-label={t("collections.rename")}
                  onClick={() => openRename(collection.id, collection.name)}
                  className="px-2"
                >
                  <Pencil size={14} aria-hidden="true" />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  aria-label={t("collections.delete")}
                  onClick={() => setDeleting(collection.id)}
                  className="px-2 text-text-secondary hover:text-text-danger"
                >
                  <Trash2 size={14} aria-hidden="true" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      </div>

      <Dialog open={renaming !== null} onOpenChange={(open) => !open && setRenaming(null)}>
        <DialogContent closeLabel={t("a11y.close")}>
          <DialogTitle>{t("collections.renameDialogTitle")}</DialogTitle>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              if (!renaming) return;
              const trimmed = renameValue.trim();
              if (!trimmed) return;
              rename.mutate(
                { collection_id: renaming.id, name: trimmed },
                {
                  onSuccess: (view) => {
                    setRenaming(null);
                    toast.success({ title: t("collections.renamedToast", { name: view.name }) });
                  },
                  onError: () => toast.error({ title: t("collections.renameError") }),
                },
              );
            }}
            className="mt-4 flex flex-col gap-4"
          >
            <InputField
              label={t("collections.fieldName")}
              value={renameValue}
              onChange={(event) => setRenameValue(event.target.value)}
            />
            <div className="flex justify-end gap-2">
              <DialogClose asChild>
                <Button variant="ghost" size="sm" onClick={() => setRenaming(null)}>
                  {t("collections.cancel")}
                </Button>
              </DialogClose>
              <Button type="submit" size="sm" disabled={!renameValue.trim() || rename.isPending}>
                {t("collections.renameSubmit")}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={deleting !== null} onOpenChange={(open) => !open && setDeleting(null)}>
        <DialogContent>
          <DialogTitle>{t("collections.deleteDialogTitle")}</DialogTitle>
          <DialogDescription>{t("collections.deleteDialogHint")}</DialogDescription>
          <div className="mt-6 flex justify-end gap-2">
            <Button variant="secondary" onClick={() => setDeleting(null)}>
              {t("collections.cancel")}
            </Button>
            <Button
              variant="danger"
              disabled={remove.isPending}
              onClick={() => {
                if (!deleting) return;
                const target = collections.find((c) => c.id === deleting);
                remove.mutate(deleting, {
                  onSuccess: () => {
                    setDeleting(null);
                    toast.success({
                      title: t("collections.deletedToast", { name: target?.name ?? "" }),
                    });
                  },
                  onError: () => toast.error({ title: t("collections.deleteError") }),
                });
              }}
              aria-label={t("collections.deleteSubmit")}
            >
              {t("collections.deleteSubmit")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </section>
  );
}

interface CreateDialogProps {
  value: string;
  onChange: (value: string) => void;
  pending: boolean;
  onSubmit: (value: string) => void;
}

function CreateDialog({ value, onChange, pending, onSubmit }: CreateDialogProps) {
  const { t } = useTranslation();
  return (
    <DialogContent closeLabel={t("a11y.close")}>
      <DialogTitle>{t("collections.createDialogTitle")}</DialogTitle>
      <DialogDescription>{t("collections.createDialogHint")}</DialogDescription>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          const trimmed = value.trim();
          if (trimmed) onSubmit(trimmed);
        }}
        className="mt-4 flex flex-col gap-4"
      >
        <InputField
          label={t("collections.fieldName")}
          placeholder={t("collections.namePlaceholder")}
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
        <div className="flex justify-end gap-2">
          <DialogClose asChild>
            <Button variant="ghost" size="sm" onClick={() => onChange("")}>
              {t("collections.cancel")}
            </Button>
          </DialogClose>
          <Button type="submit" size="sm" disabled={!value.trim() || pending}>
            {t("collections.createSubmit")}
          </Button>
        </div>
      </form>
    </DialogContent>
  );
}
