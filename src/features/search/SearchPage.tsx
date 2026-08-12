import { Search, SearchX } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { MediaRow } from "@/features/library/MediaRow";
import { useMediaSearchQuery } from "./api";

/* MISSION-043 — Local search results page. Reads `q` from the URL
   (`/search?q=...`), runs it against the FTS backend, and lists the matching
   titles as rows. The search input lives in the TopBar (single source of
   truth); this page only renders results for the query it was handed. */

function SearchSkeleton() {
  return (
    <div role="status" aria-label="Searching" className="px-6 pt-6">
      {Array.from({ length: 5 }, (_, index) => (
        <div key={index} className="mb-2 flex items-center gap-3 rounded-md px-3 py-2">
          <Skeleton className="size-10" />
          <Skeleton className="h-4 flex-1" />
          <Skeleton className="h-3 w-24" />
        </div>
      ))}
    </div>
  );
}

export function SearchPage() {
  const { t } = useTranslation();
  const [searchParams] = useSearchParams();
  const query = searchParams.get("q") ?? "";
  const trimmed = query.trim();
  const { data, isLoading, isError, refetch } = useMediaSearchQuery(trimmed);

  if (trimmed === "") {
    return (
      <EmptyState icon={Search} title={t("search.initialTitle")} hint={t("search.initialHint")} />
    );
  }

  if (isLoading) return <SearchSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={SearchX}
        title={t("search.errorTitle")}
        hint={t("search.errorHint")}
        action={
          <Button variant="secondary" onClick={() => void refetch()}>
            {t("search.retry")}
          </Button>
        }
      />
    );
  }

  const items = data ?? [];
  if (items.length === 0) {
    return (
      <EmptyState
        icon={SearchX}
        title={t("search.noResultsTitle")}
        hint={t("search.noResultsHint")}
      />
    );
  }

  return (
    <section aria-label={t("nav.search")} className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b border-border-subtle px-6 py-3 text-sm text-text-secondary">
        {t("search.resultsCount", { count: items.length, query: trimmed })}
      </div>
      <div className="flex-1 space-y-2 overflow-y-auto px-6 py-5">
        {items.map((item) => (
          <MediaRow key={item.id} item={item} />
        ))}
      </div>
    </section>
  );
}
