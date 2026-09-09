import { Star } from "lucide-react";
import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import { useAssetViews } from "@/features/library/api";
import { useReviewsQuery } from "@/features/library/review";
import { CoverImage } from "@/features/library/CoverImage";
import { Badge, Button, EmptyState, Skeleton } from "@/components/ui";

/* MISSION-144 — Reviews hub. The aggregate surface for every review row in
   the library (the per-media editor lives on the detail page): title + cover,
   rating, written review (with spoiler flag), mood/pace, and last-updated.
   Replaces the long-standing /reviews placeholder. */

function ReviewsSkeleton() {
  return (
    <div role="status" aria-label="Loading reviews" className="px-6 pt-6">
      {Array.from({ length: 4 }, (_, index) => (
        <div key={index} className="mb-2 flex items-start gap-3 rounded-md px-3 py-3">
          <Skeleton className="size-10" />
          <div className="min-w-0 flex-1">
            <Skeleton className="mb-2 h-4 w-1/3" />
            <Skeleton className="h-3 w-2/3" />
          </div>
        </div>
      ))}
    </div>
  );
}

function formatDate(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

export function ReviewsPage() {
  const { t } = useTranslation();
  const { data, isLoading, isError, refetch } = useReviewsQuery();

  const items = data ?? [];
  const covers = useAssetViews(items.map((item) => item.cover_asset_id ?? ""));

  if (isLoading) return <ReviewsSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={Star}
        title={t("reviewsPage.errorTitle")}
        hint={t("reviewsPage.errorHint")}
        action={
          <Button variant="secondary" onClick={() => void refetch()}>
            {t("reviewsPage.retry")}
          </Button>
        }
      />
    );
  }

  if (items.length === 0) {
    return (
      <EmptyState
        icon={Star}
        title={t("reviewsPage.emptyTitle")}
        hint={t("reviewsPage.emptyHint")}
      />
    );
  }

  return (
    <div className="px-6 pt-6">
      <ul className="flex flex-col gap-2">
        {items.map((item) => {
          const cover = covers.data?.find((asset) => asset.id === item.cover_asset_id);
          const shownText = item.short_review?.trim() || item.review?.trim() || null;
          return (
            <li key={item.media_id} className="flex items-start gap-3 rounded-md px-3 py-3">
              <Link
                to={`/library/${item.media_id}`}
                aria-label={t("reviewsPage.readReview", { title: item.title })}
                className="size-10 shrink-0 overflow-hidden rounded-sm"
              >
                <CoverImage
                  asset={cover}
                  contentType={item.content_type}
                  alt={item.title}
                  iconSize={20}
                />
              </Link>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <Link
                    to={`/library/${item.media_id}`}
                    dir="auto"
                    className="truncate font-medium text-text-primary hover:text-accent"
                  >
                    {item.title}
                  </Link>
                  {item.rating !== null && (
                    <span className="inline-flex items-center gap-1 text-sm tabular-nums text-text-secondary">
                      <Star size={13} className="fill-current text-warn" aria-hidden="true" />
                      {item.rating}/10
                    </span>
                  )}
                  {item.is_spoiler && (
                    <Badge variant="dropped">{t("reviewsPage.spoilerBadge")}</Badge>
                  )}
                </div>
                {shownText ? (
                  <p className="mt-1 line-clamp-2 text-sm text-text-secondary">{shownText}</p>
                ) : (
                  <p className="mt-1 text-sm italic text-text-tertiary">
                    {t("reviewsPage.noText")}
                  </p>
                )}
                <p className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-text-tertiary">
                  {item.moods.map((mood) => (
                    <span key={mood} className="rounded-full bg-bg-raised px-2 py-0.5">
                      {t(`mood.${mood}`)}
                    </span>
                  ))}
                  {item.pace && (
                    <span className="rounded-full bg-bg-raised px-2 py-0.5">
                      {t(`pace.${item.pace}`)}
                    </span>
                  )}
                  <span>{t("reviewsPage.updatedOn", { date: formatDate(item.updated_at) })}</span>
                </p>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
