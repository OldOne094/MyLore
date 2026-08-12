import { Library, Plus, RefreshCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { useMediaListQuery } from "./api";
import { AddMediaDialog } from "./AddMediaDialog";
import { MediaCard } from "./MediaCard";

/* MISSION-041 — Library landing view: the add flow when empty, a responsive
   grid when titles exist, skeletons while loading, and a retry on failure. */

function AddTitleTrigger() {
  const { t } = useTranslation();
  return (
    <AddMediaDialog
      trigger={
        <Button>
          <Plus size={16} aria-hidden="true" />
          {t("library.add")}
        </Button>
      }
    />
  );
}

function LibraryGridSkeleton() {
  return (
    <div
      aria-label="Loading library"
      role="status"
      className="grid grid-cols-2 gap-4 p-6 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6"
    >
      {Array.from({ length: 12 }, (_, index) => (
        <div key={index} className="flex flex-col gap-2">
          <Skeleton className="aspect-[2/3] w-full" />
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-3 w-1/2" />
        </div>
      ))}
    </div>
  );
}

function EmptyLibrary() {
  const { t } = useTranslation();
  return (
    <EmptyState
      icon={Library}
      title={t("library.emptyTitle")}
      hint={t("library.emptyHint")}
      action={<AddTitleTrigger />}
    />
  );
}

export function LibraryPage() {
  const { t } = useTranslation();
  const { data, isLoading, isError, refetch } = useMediaListQuery();

  if (isLoading) return <LibraryGridSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={Library}
        title={t("library.errorTitle")}
        hint={t("library.errorHint")}
        action={
          <Button variant="secondary" onClick={() => void refetch()}>
            <RefreshCcw size={16} aria-hidden="true" />
            {t("library.retry")}
          </Button>
        }
      />
    );
  }

  const items = data ?? [];
  if (items.length === 0) return <EmptyLibrary />;

  return (
    <section aria-label={t("nav.library")}>
      <div className="grid grid-cols-2 gap-4 p-6 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
        {items.map((item) => (
          <MediaCard key={item.id} item={item} />
        ))}
      </div>
    </section>
  );
}
