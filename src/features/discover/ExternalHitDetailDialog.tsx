import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  Skeleton,
} from "@/components/ui";
import type { ExternalHit } from "@/api";
import { useImportProvider } from "@/features/discover/api";
import { useProviderGetDetails } from "./detail-api";

/* MISSION-127 — External-hit detail screen. Opens as a dialog when a
   Discover result is clicked, fetching full ProviderMedia metadata from the
   provider. Shows synopsis, cover, authors/creators, genres/tags, status,
   dates, counts and external links — everything needed to judge a title
   before importing. */

function DetailSkeleton() {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex gap-4">
        <Skeleton className="aspect-[2/3] w-32 shrink-0" />
        <div className="flex flex-1 flex-col gap-2 pt-2">
          <Skeleton className="h-6 w-3/4" />
          <Skeleton className="h-4 w-1/2" />
          <Skeleton className="mt-3 h-4 w-full" />
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-2/3" />
        </div>
      </div>
      <Skeleton className="h-20 w-full" />
    </div>
  );
}

function MetaItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span className="text-xs uppercase tracking-wide text-text-tertiary">{label}</span>
      <span className="block text-sm text-text-primary">{value}</span>
    </div>
  );
}

export interface ExternalHitDetailDialogProps {
  hit: ExternalHit;
  open: boolean;
  onClose: () => void;
  onImported: () => void;
}

export function ExternalHitDetailDialog({
  hit,
  open,
  onClose,
  onImported,
}: ExternalHitDetailDialogProps) {
  const { t } = useTranslation();
  const importProvider = useImportProvider();
  const [imported, setImported] = useState(false);

  const detail = useProviderGetDetails(hit.provider, hit.provider_id, open);
  const data = detail.data;

  const handleImport = () => {
    importProvider.mutate(
      { provider: hit.provider, provider_id: hit.provider_id },
      {
        onSuccess: () => {
          setImported(true);
          onImported();
        },
      },
    );
  };

  const genres = data?.genres ?? [];
  const tags = data?.tags ?? [];
  const authors = (data?.people ?? []).filter((p) => p.role === "author" || p.role === "Author");
  const studios = (data?.people ?? []).filter((p) => p.role === "studio" || p.role === "Studio");

  return (
    <Dialog open={open} onOpenChange={(value) => !value && onClose()}>
      <DialogContent closeLabel={t("a11y.close")} className="max-w-2xl">
        <DialogTitle>{t("discover.detailTitle")}</DialogTitle>
        <DialogDescription>{hit.title}</DialogDescription>

        <div className="mt-4 max-h-[65vh] overflow-y-auto">
          {detail.isLoading ? (
            <DetailSkeleton />
          ) : detail.isError || !data ? (
            <p className="text-sm text-destructive">{t("discover.detailError")}</p>
          ) : (
            <div className="flex flex-col gap-5">
              {/* Hero: cover + title + key meta */}
              <div className="flex gap-4">
                {data.cover_url ? (
                  <img
                    src={data.cover_url}
                    alt=""
                    className="aspect-[2/3] w-28 shrink-0 rounded-md object-cover"
                    loading="lazy"
                  />
                ) : null}
                <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                  <h3 dir="auto" className="text-lg font-semibold text-text-primary">
                    {data.title_main}
                  </h3>
                  {data.title_original ? (
                    <p dir="auto" className="text-sm text-text-secondary">
                      {data.title_original}
                    </p>
                  ) : null}
                  <div className="flex flex-wrap items-center gap-2 text-xs text-text-tertiary">
                    {data.release_year ? <span>{data.release_year}</span> : null}
                    {data.format ? <span>{data.format}</span> : null}
                    {data.pub_status !== "unknown" ? (
                      <span>
                        {t(`pubStatus.${data.pub_status}`, { defaultValue: data.pub_status })}
                      </span>
                    ) : null}
                  </div>
                </div>
              </div>

              {/* Synopsis */}
              {data.synopsis ? (
                <p
                  dir="auto"
                  className="max-h-40 overflow-y-auto text-sm leading-relaxed text-text-secondary"
                >
                  {data.synopsis}
                </p>
              ) : null}

              {/* People */}
              {authors.length > 0 ? (
                <MetaItem
                  label={t("discover.detailAuthors")}
                  value={authors.map((a) => a.name).join(", ")}
                />
              ) : null}
              {studios.length > 0 ? (
                <MetaItem
                  label={t("discover.detailStudios")}
                  value={studios.map((s) => s.name).join(", ")}
                />
              ) : null}

              {/* Genres + tags */}
              {(genres.length > 0 || tags.length > 0) && (
                <div className="flex flex-wrap gap-1.5">
                  {genres.map((g) => (
                    <span
                      key={g}
                      className="rounded-full border border-border-subtle px-2 py-0.5 text-xs text-text-secondary"
                    >
                      {g}
                    </span>
                  ))}
                  {tags.map((tag) => (
                    <span
                      key={tag}
                      className="rounded-full bg-bg-hover px-2 py-0.5 text-xs text-text-tertiary"
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              )}

              {/* Counts */}
              <div className="grid grid-cols-3 gap-x-4 gap-y-2">
                {data.pages ? (
                  <MetaItem label={t("discover.detailPages")} value={String(data.pages)} />
                ) : null}
                {data.ep_count ? (
                  <MetaItem label={t("discover.detailEpisodes")} value={String(data.ep_count)} />
                ) : null}
                {data.ch_count ? (
                  <MetaItem label={t("discover.detailChapters")} value={String(data.ch_count)} />
                ) : null}
              </div>

              {/* External link */}
              {data.url ? (
                <a
                  href={data.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-xs text-accent transition-colors duration-150 ease-out hover:text-accent-hover"
                >
                  {t("discover.detailExternalLink")}
                </a>
              ) : null}
            </div>
          )}

          <div className="mt-4 flex justify-end gap-2">
            <DialogClose asChild>
              <Button variant="secondary">{t("discover.close")}</Button>
            </DialogClose>
            {!imported ? (
              <Button onClick={handleImport} disabled={importProvider.isPending}>
                {importProvider.isPending ? t("discover.importing") : t("discover.import")}
              </Button>
            ) : (
              <Button disabled>{t("discover.imported")}</Button>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
