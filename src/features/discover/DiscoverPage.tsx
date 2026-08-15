import { ArrowUpRight, Compass, Search, SearchX } from "lucide-react";
import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate } from "react-router";
import { Badge, Button, EmptyState, Skeleton } from "@/components/ui";
import { useToast } from "@/components/ui";
import { MediaRow } from "@/features/library/MediaRow";
import { useDiscoverSearchQuery, useImportProvider } from "./api";

/* MISSION-059 — External search (Discover). Searches every enabled provider
   through the coordinator, groups hits by provider, and flags each hit as
   already-in-library / duplicate / new via the identity service. Local library
   matches for the same query are listed above the provider groups. */

const CONTENT_TYPES = [
  "book",
  "novel",
  "web_novel",
  "manga",
  "manhwa",
  "manhua",
  "anime",
  "tv",
  "movie",
  "other",
] as const;

const IDENTITY_VARIANT: Record<string, "accent" | "neutral" | "planned"> = {
  in_library: "accent",
  duplicate: "planned",
  new: "neutral",
};

function DiscoverSkeleton() {
  return (
    <div role="status" aria-label="Searching providers" className="space-y-6">
      {Array.from({ length: 3 }, (_, group) => (
        <div key={group}>
          <Skeleton className="mb-2 h-4 w-40" />
          {Array.from({ length: 2 }, (_, row) => (
            <div key={row} className="mb-2 flex items-center gap-3 rounded-md px-3 py-2">
              <Skeleton className="size-10" />
              <Skeleton className="h-4 flex-1" />
              <Skeleton className="h-3 w-24" />
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function IdentityBadge({ kind }: { kind: string }) {
  const { t } = useTranslation();
  const key =
    kind === "in_library"
      ? "discover.inLibrary"
      : kind === "duplicate"
        ? "discover.duplicate"
        : "discover.new";
  return <Badge variant={IDENTITY_VARIANT[kind] ?? "neutral"}>{t(key)}</Badge>;
}

function ExternalHitRow({ hit }: { hit: import("@/api").ExternalHit }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const toast = useToast();
  const importProvider = useImportProvider();

  const title =
    hit.identity.kind === "in_library" && hit.identity.media_id ? (
      <Link
        to={`/library/${hit.identity.media_id}`}
        className="flex items-center gap-1 text-text-primary hover:text-accent"
      >
        <span className="truncate">{hit.title}</span>
        <ArrowUpRight size={14} aria-hidden="true" />
      </Link>
    ) : (
      <span className="truncate">{hit.title}</span>
    );

  const alreadyAdded = hit.identity.kind === "in_library";
  const onImport = () => {
    importProvider.mutate(
      { provider: hit.provider, provider_id: hit.provider_id },
      {
        onSuccess: (view) => {
          if (view.created) {
            toast.success({ title: t("discover.importedToast", { title: view.title }) });
          } else if (view.identity_kind === "duplicate") {
            toast.info({ title: t("discover.importDuplicate", { title: view.title }) });
          } else {
            toast.info({ title: t("discover.importAlreadyAdded", { title: view.title }) });
          }
          navigate(`/library/${view.media_id}`);
        },
        onError: () => toast.error({ title: t("discover.importError", { title: hit.title }) }),
      },
    );
  };

  return (
    <div className="flex items-center gap-3 rounded-md border border-transparent px-3 py-2 transition-colors duration-150 ease-out hover:border-border-subtle hover:bg-bg-hover">
      <div className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-sm bg-bg-hover">
        {hit.cover_url ? (
          <img src={hit.cover_url} alt="" loading="lazy" className="size-full object-cover" />
        ) : (
          <Search size={18} aria-hidden="true" className="text-text-tertiary" />
        )}
      </div>
      {title}
      <Badge variant="neutral" className="hidden sm:inline-flex">
        {t(`contentType.${hit.content_type}`)}
      </Badge>
      {hit.release_year ? (
        <span className="shrink-0 text-xs tabular-nums text-text-tertiary">{hit.release_year}</span>
      ) : null}
      <IdentityBadge kind={hit.identity.kind} />
      {!alreadyAdded ? (
        <Button
          size="sm"
          variant="secondary"
          onClick={onImport}
          disabled={importProvider.isPending}
          className="ml-auto shrink-0"
        >
          {importProvider.isPending ? t("discover.importing") : t("discover.import")}
        </Button>
      ) : null}
    </div>
  );
}

export function DiscoverPage() {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");
  const [contentType, setContentType] = useState<string | null>(null);

  const trimmed = query.trim();
  const { data, isLoading, isError, refetch } = useDiscoverSearchQuery(trimmed, contentType);

  function submit(event: FormEvent) {
    event.preventDefault();
    setQuery(draft);
  }

  return (
    <section aria-label={t("nav.discover")} className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b border-border-subtle px-6 py-4">
        <form
          role="search"
          onSubmit={submit}
          className="flex items-end gap-3"
          aria-label={t("discover.inputLabel")}
        >
          <div className="flex min-w-0 flex-1 flex-col gap-1.5">
            <label htmlFor="discover-query" className="text-sm font-medium text-text-secondary">
              {t("discover.inputLabel")}
            </label>
            <input
              id="discover-query"
              type="search"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              placeholder={t("discover.placeholder")}
              className="h-[var(--control-height)] w-full rounded-sm border bg-bg-base px-3 text-base text-text-primary placeholder:text-text-tertiary transition-colors duration-150 ease-out hover:border-accent focus-visible:outline-none"
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="discover-type" className="text-sm font-medium text-text-secondary">
              {t("discover.allTypes")}
            </label>
            <select
              id="discover-type"
              value={contentType ?? ""}
              onChange={(event) => setContentType(event.target.value || null)}
              className="h-[var(--control-height)] rounded-sm border bg-bg-base px-3 text-base text-text-primary transition-colors duration-150 ease-out hover:border-accent focus-visible:outline-none"
            >
              <option value="">{t("discover.allTypes")}</option>
              {CONTENT_TYPES.map((type) => (
                <option key={type} value={type}>
                  {t(`contentType.${type}`)}
                </option>
              ))}
            </select>
          </div>
          <Button type="submit" size="md" className="shrink-0">
            <Search size={16} aria-hidden="true" />
            {t("discover.searchButton")}
          </Button>
        </form>
        <p className="mt-2 text-xs text-text-tertiary">{t("discover.inputHint")}</p>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        {trimmed === "" ? (
          <EmptyState
            icon={Compass}
            title={t("discover.initialTitle")}
            hint={t("discover.initialHint")}
          />
        ) : null}

        {trimmed !== "" && isLoading ? <DiscoverSkeleton /> : null}

        {trimmed !== "" && isError ? (
          <EmptyState
            icon={SearchX}
            title={t("discover.errorTitle")}
            hint={t("discover.errorHint")}
            action={
              <Button variant="secondary" onClick={() => void refetch()}>
                {t("discover.retry")}
              </Button>
            }
          />
        ) : null}

        {trimmed !== "" && data ? (
          <div className="space-y-6">
            <div className="shrink-0 text-sm text-text-secondary">
              {t("discover.resultsCount", {
                count: data.local.length + data.groups.reduce((n, g) => n + g.hits.length, 0),
                query: trimmed,
              })}
            </div>

            {data.local.length > 0 ? (
              <section aria-label={t("discover.localSection")}>
                <h2 className="mb-2 text-sm font-semibold text-text-primary">
                  {t("discover.localSection")}
                </h2>
                <div className="space-y-2">
                  {data.local.map((item) => (
                    <MediaRow key={item.id} item={item} />
                  ))}
                </div>
              </section>
            ) : null}

            {data.groups.map((group) => (
              <section key={group.provider} aria-label={group.name}>
                <h2 className="mb-2 flex items-center gap-2 text-sm font-semibold text-text-primary">
                  {group.name}
                  <span className="text-xs font-normal text-text-tertiary">
                    {group.hits.length}
                  </span>
                </h2>
                <div className="space-y-1">
                  {group.hits.map((hit) => (
                    <ExternalHitRow key={`${hit.provider}-${hit.provider_id}`} hit={hit} />
                  ))}
                </div>
              </section>
            ))}

            {data.groups.length === 0 && data.local.length === 0 ? (
              <EmptyState
                icon={SearchX}
                title={t("discover.noResultsTitle")}
                hint={t("discover.noResultsHint")}
              />
            ) : null}

            {data.failures.length > 0 ? (
              <div className="border-t border-border-subtle pt-3">
                <ul className="space-y-1 text-xs text-text-tertiary">
                  {data.failures.map((failure) => (
                    <li key={failure.provider}>
                      {t("discover.providerError", { provider: failure.provider })}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
          </div>
        ) : null}
      </div>
    </section>
  );
}
