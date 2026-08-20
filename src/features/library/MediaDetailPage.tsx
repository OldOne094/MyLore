import { useState } from "react";
import {
  ArrowLeft,
  Check,
  FileText,
  ListTree,
  RefreshCcw,
  Star,
  Activity,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate, useParams } from "react-router";
import {
  Badge,
  Button,
  EmptyState,
  Skeleton,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  useToast,
} from "@/components/ui";
import { useMediaDetailQuery, useAssetViews, useEnrichMedia } from "./api";
import type { EnrichView } from "@/api";
import { NodeTree } from "./NodeTree";
import { TrackingTab } from "./TrackingTab";
import { ReviewTab } from "./ReviewTab";
import { useAcknowledgeWarnings, useReviewQuery } from "./review";
import { EnrichDialog } from "./EnrichDialog";
import { useDeleteMedia, useRestoreTrashItem } from "@/features/trash/api";
import { STATUS_VARIANTS } from "./mediaMeta";
import { CoverImage } from "./CoverImage";

/* MISSION-042 — Media detail page. Hero (cover, title, meta badges, actions)
   above tabbed sections: Overview / Details / Tracking / Review. Overview and
   Details render the aggregate fields; Tracking is the MISSION-048 status
   picker; Review is the MISSION-074 rating/review/notes/tags editor. MISSION-079
   adds the user-owned StoryGraph-style badges to the hero — mood / pace chips
   and content-warning badges with a one-tap acknowledgment stamp. */

type DetailTab = "overview" | "details" | "tracking" | "review";

const TABS: DetailTab[] = ["overview", "details", "tracking", "review"];

const TAB_ICONS: Record<DetailTab, typeof FileText> = {
  overview: FileText,
  details: ListTree,
  tracking: Activity,
  review: Star,
};

function prettyId(raw: string): string {
  return raw.replace(/_/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());
}

function DetailSkeleton() {
  return (
    <div className="flex flex-col gap-6 px-6 py-6">
      <div className="flex gap-5">
        <Skeleton className="h-56 w-40" />
        <div className="flex flex-1 flex-col gap-3 pt-2">
          <Skeleton className="h-7 w-2/3" />
          <Skeleton className="h-4 w-1/3" />
          <Skeleton className="mt-2 h-4 w-1/2" />
          <Skeleton className="h-4 w-3/4" />
        </div>
      </div>
      <Skeleton className="h-10 w-full" />
      <Skeleton className="h-40 w-full" />
    </div>
  );
}

function MetaCell({ label, value }: { label: string; value: string }) {
  if (!value) return null;
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-xs uppercase tracking-wide text-text-tertiary">{label}</span>
      <span className="text-sm text-text-primary">{value}</span>
    </div>
  );
}

export function MediaDetailPage() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const toast = useToast();
  const [tab, setTab] = useState<DetailTab>("overview");
  const [enrichView, setEnrichView] = useState<EnrichView | null>(null);
  const { data, isPending, isError, refetch } = useMediaDetailQuery(id ?? "");
  const { data: coverViews } = useAssetViews(data?.cover_asset_id ? [data.cover_asset_id] : []);
  const cover = coverViews?.[0] ?? null;
  const { data: review } = useReviewQuery(data?.id ?? "", data != null);
  const deleteMedia = useDeleteMedia();
  const restoreTrashItem = useRestoreTrashItem();
  const enrichMedia = useEnrichMedia();
  const acknowledgeWarnings = useAcknowledgeWarnings();

  const handleAcknowledge = () => {
    if (!review) return;
    acknowledgeWarnings.mutate(review.media_id, {
      onSuccess: () => toast.success({ title: t("review.warningsAcknowledgeToast") }),
      onError: () => toast.error({ title: t("review.warningsAcknowledgeErrorToast") }),
    });
  };

  const handleEnrich = () => {
    if (!data) return;
    enrichMedia.mutate(data.id, {
      onSuccess: (view) => {
        if (view.changed) {
          setEnrichView(view);
          toast.success({
            title: t("enrich.updatedToast", { title: data.title_main, provider: view.provider }),
          });
        } else {
          toast.success({ title: t("enrich.unchangedToast", { title: data.title_main }) });
        }
      },
      onError: () => toast.error({ title: t("enrich.errorToast", { title: data.title_main }) }),
    });
  };

  const handleDelete = () => {
    if (!data) return;
    deleteMedia.mutate(data.id, {
      onSuccess: (trashId) => {
        navigate("/library");
        toast.success({
          title: t("trash.deletedToast", { count: 1 }),
          action: {
            label: t("trash.undo"),
            onClick: () => {
              void restoreTrashItem.mutateAsync(trashId).then(
                () =>
                  toast.success({
                    title: t("trash.restoredToast", { title: data.title_main }),
                  }),
                () => toast.error({ title: t("trash.restoreErrorToast") }),
              );
            },
          },
        });
      },
      onError: () => toast.error({ title: t("trash.deleteErrorToast") }),
    });
  };

  if (isPending) return <DetailSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={RefreshCcw}
        title={t("detail.errorTitle")}
        hint={t("detail.errorHint")}
        action={<Button onClick={() => refetch()}>{t("detail.retry")}</Button>}
      />
    );
  }

  if (!data) {
    return (
      <EmptyState
        icon={RefreshCcw}
        title={t("detail.notFoundTitle")}
        hint={t("detail.notFoundHint")}
      />
    );
  }

  const title = data.title_main;

  return (
    <section aria-label={title} className="flex h-full min-h-0 flex-col overflow-y-auto">
      <div className="flex flex-col gap-6 px-6 pb-8 pt-6">
        <Link
          to="/library"
          className="inline-flex items-center gap-1.5 text-sm text-text-secondary transition-colors duration-150 ease-out hover:text-text-primary"
        >
          <ArrowLeft size={16} aria-hidden="true" className="rtl:rotate-180" />
          {t("detail.backToLibrary")}
        </Link>

        <div className="flex flex-col gap-6 md:flex-row">
          <div className="relative aspect-[2/3] w-40 shrink-0 overflow-hidden rounded-lg border border-border-subtle bg-bg-hover text-text-tertiary">
            <CoverImage
              asset={cover}
              contentType={data.content_type}
              alt={title}
              iconSize={48}
              className="absolute inset-0"
              imgClassName="absolute inset-0"
            />
          </div>

          <div className="flex min-w-0 flex-1 flex-col gap-3">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h1 className="text-2xl font-semibold text-text-primary">{title}</h1>
                {data.title_original ? (
                  <p className="mt-1 text-sm text-text-tertiary">{data.title_original}</p>
                ) : null}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {data.provider ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={handleEnrich}
                    disabled={enrichMedia.isPending}
                    aria-label={t("enrich.refreshAria", { title, provider: data.provider })}
                  >
                    <RefreshCcw
                      size={14}
                      aria-hidden="true"
                      className={enrichMedia.isPending ? "animate-spin" : undefined}
                    />
                    {enrichMedia.isPending ? t("enrich.refreshing") : t("enrich.refresh")}
                  </Button>
                ) : null}
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={handleDelete}
                  disabled={deleteMedia.isPending || restoreTrashItem.isPending}
                  aria-label={t("trash.deleteAria", { title })}
                >
                  <Trash2 size={14} aria-hidden="true" />
                  {t("trash.delete")}
                </Button>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="accent">{t(`contentType.${data.content_type}`)}</Badge>
              <Badge variant={STATUS_VARIANTS[data.pub_status] ?? "neutral"}>
                {t(`pubStatus.${data.pub_status}`)}
              </Badge>
              {data.release_year ? (
                <span className="text-sm tabular-nums text-text-tertiary">{data.release_year}</span>
              ) : null}
              {review?.pace ? <Badge variant="neutral">{t(`pace.${review.pace}`)}</Badge> : null}
              {(review?.moods ?? []).map((mood) => (
                <Badge key={mood} variant="accent">
                  {t(`mood.${mood}`)}
                </Badge>
              ))}
              {(review?.content_warnings?.length ?? 0) > 0 && review ? (
                <span className="flex flex-wrap items-center gap-2">
                  {review.content_warnings.map((warning) => (
                    <Badge
                      key={warning}
                      variant="neutral"
                      className="border-danger/30 bg-danger/10 text-danger"
                    >
                      {t(`warning.${warning}`)}
                    </Badge>
                  ))}
                  {review.warnings_acknowledged_at ? (
                    <span className="inline-flex items-center gap-1 text-xs text-text-tertiary">
                      <Check size={12} aria-hidden="true" />
                      {t("review.warningsAcknowledged")}
                    </span>
                  ) : (
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={acknowledgeWarnings.isPending}
                      onClick={handleAcknowledge}
                      aria-label={t("review.warningsAcknowledgeAria", {
                        title: data.title_main,
                      })}
                    >
                      {t("review.warningsAcknowledge")}
                    </Button>
                  )}
                </span>
              ) : null}
            </div>
            {data.synopsis ? (
              <p className="max-w-2xl text-sm leading-relaxed text-text-secondary">
                {data.synopsis}
              </p>
            ) : null}
          </div>
        </div>

        <Tabs value={tab} onValueChange={(value) => setTab(value as DetailTab)}>
          <TabsList ariaLabel={title}>
            {TABS.map((value) => {
              const TabIcon = TAB_ICONS[value];
              return (
                <TabsTrigger key={value} value={value}>
                  <span className="inline-flex items-center gap-1.5">
                    <TabIcon size={14} aria-hidden="true" />
                    {t(`detail.${value}`)}
                  </span>
                </TabsTrigger>
              );
            })}
          </TabsList>

          <TabsContent value="overview" className="pt-6">
            <div className="grid grid-cols-2 gap-x-8 gap-y-5 sm:grid-cols-3 lg:grid-cols-4">
              <MetaCell
                label={t("detail.metaFormat")}
                value={data.format ? prettyId(data.format) : ""}
              />
              <MetaCell label={t("detail.metaStatus")} value={t(`pubStatus.${data.pub_status}`)} />
              <MetaCell
                label={t("detail.metaYear")}
                value={data.release_year ? String(data.release_year) : ""}
              />
              <MetaCell label={t("detail.metaLanguage")} value={data.language ?? ""} />
              <MetaCell label={t("detail.metaCountry")} value={data.country ?? ""} />
              {data.pages ? (
                <MetaCell label={t("detail.metaPages")} value={String(data.pages)} />
              ) : null}
              {data.ep_count ? (
                <MetaCell label={t("detail.metaEpisodes")} value={String(data.ep_count)} />
              ) : null}
              {data.ch_count ? (
                <MetaCell label={t("detail.metaChapters")} value={String(data.ch_count)} />
              ) : null}
              {data.duration_min ? (
                <MetaCell label={t("detail.metaDuration")} value={`${data.duration_min} min`} />
              ) : null}
            </div>
            {data.genres.length > 0 ? (
              <div className="mt-8 flex flex-col gap-2">
                <h2 className="text-xs uppercase tracking-wide text-text-tertiary">
                  {t("detail.metaGenres")}
                </h2>
                <div className="flex flex-wrap gap-1.5">
                  {data.genres.map((genre) => (
                    <Badge key={genre} variant="neutral">
                      {prettyId(genre)}
                    </Badge>
                  ))}
                </div>
              </div>
            ) : null}
          </TabsContent>

          <TabsContent value="details" className="pt-6">
            <NodeTree mediaId={data.id} mediaTitle={title} contentType={data.content_type} />
          </TabsContent>

          <TabsContent value="tracking" className="pt-6">
            <TrackingTab mediaId={data.id} />
          </TabsContent>

          <TabsContent value="review" className="pt-6">
            <ReviewTab mediaId={data.id} />
          </TabsContent>
        </Tabs>
      </div>
      {enrichView ? (
        <EnrichDialog
          view={enrichView}
          open
          onOpenChange={(open) => {
            if (!open) setEnrichView(null);
          }}
        />
      ) : null}
    </section>
  );
}
