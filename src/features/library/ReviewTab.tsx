import { Heart, Loader2, Plus, RefreshCcw, Star, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ReviewView } from "@/api";
import {
  Badge,
  Button,
  EmptyState,
  InputField,
  Skeleton,
  TextareaField,
  useToast,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import { CONTENT_WARNINGS, MOODS, PACES } from "./reviewMeta";
import {
  useAddMediaTag,
  useDeleteReview,
  useMediaTagsQuery,
  useRemoveMediaTag,
  useReviewQuery,
  useSaveReview,
} from "./review";

/* MISSION-074 — Review tab. The user-owned record for a title: favorite flag,
   a 1–10 rating (star picker), a full review (with an optional spoiler flag),
   a short review, private notes, and personal tags. MISSION-079 adds the
   StoryGraph-style metadata — mood (multi), pace (single) and content-warning
   (multi) chips from fixed vocabularies. Everything saves through `review_save`
   (which validates the domain invariants server-side and clears the row when
   the review becomes empty) and the personal-tag commands. Content warnings
   are acknowledged (with a timestamp) on the detail page, never here. */

const MAX_RATING = 10;

interface Draft {
  rating: number | null;
  review: string;
  short_review: string;
  notes: string;
  favorite: boolean;
  is_spoiler: boolean;
  moods: string[];
  pace: string | null;
  content_warnings: string[];
}

const EMPTY_DRAFT: Draft = {
  rating: null,
  review: "",
  short_review: "",
  notes: "",
  favorite: false,
  is_spoiler: false,
  moods: [],
  pace: null,
  content_warnings: [],
};

function formatDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

function ReviewSkeleton() {
  return (
    <div role="status" aria-label="review" className="flex flex-col gap-6">
      <div className="flex flex-col gap-2">
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-10 w-64" />
      </div>
      <Skeleton className="h-24 w-full" />
      <Skeleton className="h-24 w-full" />
      <Skeleton className="h-8 w-40" />
    </div>
  );
}

function SectionHeading({
  title,
  hint,
  children,
}: {
  title: string;
  hint: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="flex flex-col gap-0.5">
        <h2 className="text-sm font-medium text-text-primary">{title}</h2>
        <p className="text-xs text-text-secondary">{hint}</p>
      </div>
      {children}
    </div>
  );
}

export function ReviewTab({ mediaId }: { mediaId: string }) {
  const { t } = useTranslation();
  const { data, isPending, isError, refetch } = useReviewQuery(mediaId);
  const tags = useMediaTagsQuery(mediaId);
  const save = useSaveReview();
  const clear = useDeleteReview();
  const addTag = useAddMediaTag();
  const removeTag = useRemoveMediaTag();
  const toast = useToast();

  const [draft, setDraft] = useState<Draft>(EMPTY_DRAFT);
  const [tagInput, setTagInput] = useState("");

  // Rehydrate the editor when a (different) review row arrives — a render-phase
  // state reset, so the draft never fights an in-flight server value.
  const [loadedFor, setLoadedFor] = useState<ReviewView | null | undefined>(data);
  if (data !== loadedFor) {
    setLoadedFor(data);
    setDraft(
      data
        ? {
            rating: data.rating,
            review: data.review ?? "",
            short_review: data.short_review ?? "",
            notes: data.notes ?? "",
            favorite: data.favorite,
            is_spoiler: data.is_spoiler,
            moods: data.moods,
            pace: data.pace,
            content_warnings: data.content_warnings,
          }
        : EMPTY_DRAFT,
    );
  }

  if (isPending) return <ReviewSkeleton />;

  if (isError) {
    return (
      <EmptyState
        icon={RefreshCcw}
        title={t("review.loadErrorTitle")}
        hint={t("review.loadErrorHint")}
        action={<Button onClick={() => refetch()}>{t("review.retry")}</Button>}
      />
    );
  }

  const hasReview = data != null;
  const busy = save.isPending || clear.isPending;

  const handleSave = () => {
    save.mutate(
      {
        media_id: mediaId,
        rating: draft.rating,
        review: draft.review.trim() || null,
        short_review: draft.short_review.trim() || null,
        notes: draft.notes.trim() || null,
        favorite: draft.favorite,
        is_spoiler: draft.is_spoiler,
        moods: draft.moods,
        pace: draft.pace,
        content_warnings: draft.content_warnings,
      },
      {
        onSuccess: () => toast.success({ title: t("review.savedToast") }),
        onError: () => toast.error({ title: t("review.saveErrorToast") }),
      },
    );
  };

  const handleClear = () => {
    clear.mutate(mediaId, {
      onSuccess: () => setDraft(EMPTY_DRAFT),
      onError: () => toast.error({ title: t("review.clearErrorToast") }),
    });
  };

  const handleAddTag = () => {
    const tag = tagInput.trim();
    if (!tag || addTag.isPending) return;
    addTag.mutate(
      { media_id: mediaId, tag },
      {
        onSuccess: () => {
          setTagInput("");
          toast.success({ title: t("review.tagAddedToast") });
        },
        onError: () => toast.error({ title: t("review.tagAddErrorToast") }),
      },
    );
  };

  const handleRemoveTag = (tagId: string) => {
    removeTag.mutate(
      { media_id: mediaId, tag_id: tagId },
      { onError: () => toast.error({ title: t("review.tagRemoveErrorToast") }) },
    );
  };

  const tagList = tags.data ?? [];

  const toggleKey = (key: string, list: string[]) =>
    list.includes(key) ? list.filter((item) => item !== key) : [...list, key];

  return (
    <div className="flex max-w-xl flex-col gap-8">
      <section aria-labelledby="review-favorite-heading">
        <SectionHeading title={t("review.favoriteLabel")} hint={t("review.favoriteHint")}>
          <button
            type="button"
            aria-pressed={draft.favorite}
            aria-label={t("review.favoriteLabel")}
            disabled={busy}
            onClick={() => setDraft((d) => ({ ...d, favorite: !d.favorite }))}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-sm transition-colors duration-150 ease-out disabled:opacity-60",
              draft.favorite
                ? "border-danger/50 bg-danger/10 text-danger"
                : "border-border-subtle text-text-secondary hover:border-border-strong hover:text-text-primary",
            )}
          >
            <Heart
              size={14}
              className={draft.favorite ? "fill-current" : undefined}
              aria-hidden="true"
            />
            {t("review.favoriteLabel")}
          </button>
        </SectionHeading>
      </section>

      <section aria-labelledby="review-rating-heading">
        <SectionHeading title={t("review.ratingTitle")} hint={t("review.ratingHint")} />
        <div
          role="group"
          aria-label={t("review.ratingTitle")}
          className="mt-3 flex items-center gap-0.5"
        >
          {Array.from({ length: MAX_RATING }, (_, index) => {
            const value = index + 1;
            const selected = draft.rating != null && value <= draft.rating;
            return (
              <button
                key={value}
                type="button"
                aria-label={String(value)}
                aria-pressed={draft.rating === value}
                disabled={busy}
                onClick={() =>
                  setDraft((d) => ({ ...d, rating: d.rating === value ? null : value }))
                }
                className={cn(
                  "rounded-sm p-1 text-text-tertiary transition-colors duration-150 ease-out hover:text-accent focus-visible:outline-none disabled:opacity-60",
                  selected && "text-accent",
                )}
              >
                <Star
                  size={20}
                  className={selected ? "fill-current" : undefined}
                  strokeWidth={selected ? 1.5 : 2}
                  aria-hidden="true"
                />
              </button>
            );
          })}
          {draft.rating ? (
            <span className="ms-2 text-sm font-medium tabular-nums text-text-secondary">
              {draft.rating}/10
            </span>
          ) : null}
        </div>
      </section>

      <section aria-labelledby="review-text-heading" className="flex flex-col gap-5">
        <div className="flex flex-col gap-0.5">
          <div className="flex items-center gap-2">
            <h2 id="review-text-heading" className="text-sm font-medium text-text-primary">
              {t("review.reviewLabel")}
            </h2>
            {draft.is_spoiler && (draft.review.trim() || draft.short_review.trim()) ? (
              <Badge variant="accent">{t("review.spoiler")}</Badge>
            ) : null}
          </div>
          <label className="mt-1 flex cursor-pointer items-center gap-2 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={draft.is_spoiler}
              disabled={busy}
              onChange={(event) => setDraft((d) => ({ ...d, is_spoiler: event.target.checked }))}
              className="size-3.5 accent-[var(--color-accent)]"
            />
            {t("review.spoiler")} — {t("review.spoilerHint")}
          </label>
        </div>
        <TextareaField
          label={t("review.reviewLabel")}
          value={draft.review}
          onChange={(event) => setDraft((d) => ({ ...d, review: event.target.value }))}
          placeholder={t("review.reviewPlaceholder")}
        />
        <InputField
          label={t("review.shortReviewLabel")}
          value={draft.short_review}
          onChange={(event) => setDraft((d) => ({ ...d, short_review: event.target.value }))}
          placeholder={t("review.shortReviewPlaceholder")}
        />
        <TextareaField
          label={t("review.notesLabel")}
          value={draft.notes}
          onChange={(event) => setDraft((d) => ({ ...d, notes: event.target.value }))}
          placeholder={t("review.notesPlaceholder")}
        />
      </section>

      <section aria-labelledby="review-tags-heading">
        <SectionHeading title={t("review.tagsTitle")} hint={t("review.tagsHint")} />
        <div className="mt-3 flex flex-wrap items-center gap-2">
          {tagList.map((tag) => (
            <Badge key={tag.id} variant="neutral" className="pe-1.5">
              {tag.name}
              <button
                type="button"
                aria-label={`${t("review.tagRemovedToast")}: ${tag.name}`}
                disabled={removeTag.isPending}
                onClick={() => handleRemoveTag(tag.id)}
                className="rounded-full p-0.5 text-text-tertiary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary disabled:opacity-60"
              >
                <X size={12} aria-hidden="true" />
              </button>
            </Badge>
          ))}
          <form
            onSubmit={(event) => {
              event.preventDefault();
              handleAddTag();
            }}
            className="flex items-center gap-1.5"
          >
            <InputField
              label={t("review.tagsTitle")}
              value={tagInput}
              onChange={(event) => setTagInput(event.target.value)}
              placeholder={t("review.tagPlaceholder")}
              className="h-[var(--control-height-compact)] w-40 px-2 text-sm"
              aria-label={t("review.tagPlaceholder")}
            />
            <Button
              type="submit"
              variant="secondary"
              size="sm"
              disabled={!tagInput.trim() || addTag.isPending}
              aria-label={t("review.tagAdd")}
            >
              {addTag.isPending ? (
                <Loader2 size={14} className="animate-spin" aria-hidden="true" />
              ) : (
                <Plus size={14} aria-hidden="true" />
              )}
              {t("review.tagAdd")}
            </Button>
          </form>
        </div>
      </section>

      <section aria-labelledby="review-mood-heading">
        <SectionHeading title={t("review.moodTitle")} hint={t("review.moodHint")} />
        <div role="group" aria-label={t("review.moodTitle")} className="mt-3 flex flex-wrap gap-2">
          {MOODS.map((mood) => {
            const selected = draft.moods.includes(mood);
            return (
              <button
                key={mood}
                type="button"
                aria-pressed={selected}
                disabled={busy}
                onClick={() => setDraft((d) => ({ ...d, moods: toggleKey(mood, d.moods) }))}
                className={cn(
                  "rounded-full border px-3 py-1.5 text-sm transition-colors duration-150 ease-out disabled:opacity-60",
                  selected
                    ? "border-accent/40 bg-accent-soft text-accent"
                    : "border-border-subtle text-text-secondary hover:border-border-strong hover:text-text-primary",
                )}
              >
                {t(`mood.${mood}`)}
              </button>
            );
          })}
        </div>
      </section>

      <section aria-labelledby="review-pace-heading">
        <SectionHeading title={t("review.paceTitle")} hint={t("review.paceHint")} />
        <div role="group" aria-label={t("review.paceTitle")} className="mt-3 flex flex-wrap gap-2">
          {PACES.map((pace) => {
            const selected = draft.pace === pace;
            return (
              <button
                key={pace}
                type="button"
                aria-pressed={selected}
                disabled={busy}
                onClick={() => setDraft((d) => ({ ...d, pace: selected ? null : pace }))}
                className={cn(
                  "rounded-full border px-3 py-1.5 text-sm transition-colors duration-150 ease-out disabled:opacity-60",
                  selected
                    ? "border-accent/40 bg-accent-soft text-accent"
                    : "border-border-subtle text-text-secondary hover:border-border-strong hover:text-text-primary",
                )}
              >
                {t(`pace.${pace}`)}
              </button>
            );
          })}
        </div>
      </section>

      <section aria-labelledby="review-warnings-heading">
        <SectionHeading title={t("review.warningsTitle")} hint={t("review.warningsHint")} />
        <div
          role="group"
          aria-label={t("review.warningsTitle")}
          className="mt-3 flex flex-wrap gap-2"
        >
          {CONTENT_WARNINGS.map((warning) => {
            const selected = draft.content_warnings.includes(warning);
            return (
              <button
                key={warning}
                type="button"
                aria-pressed={selected}
                disabled={busy}
                onClick={() =>
                  setDraft((d) => ({
                    ...d,
                    content_warnings: toggleKey(warning, d.content_warnings),
                  }))
                }
                className={cn(
                  "rounded-full border px-3 py-1.5 text-sm transition-colors duration-150 ease-out disabled:opacity-60",
                  selected
                    ? "border-danger/40 bg-danger/10 text-danger"
                    : "border-border-subtle text-text-secondary hover:border-border-strong hover:text-text-primary",
                )}
              >
                {t(`warning.${warning}`)}
              </button>
            );
          })}
        </div>
      </section>

      <section className="flex flex-col gap-3 border-t border-border-subtle pt-5">
        <div className="flex items-center gap-3">
          <Button size="sm" disabled={busy} onClick={handleSave}>
            {save.isPending ? (
              <Loader2 size={14} className="animate-spin" aria-hidden="true" />
            ) : null}
            {t("review.save")}
          </Button>
          {hasReview ? (
            <Button variant="ghost" size="sm" disabled={busy} onClick={handleClear}>
              {t("review.clear")}
            </Button>
          ) : null}
          {hasReview ? (
            <span className="text-xs text-text-tertiary">
              {t("review.savedAt", { date: formatDate(data.updated_at) })}
            </span>
          ) : null}
        </div>
        {!hasReview ? (
          <p className="text-xs text-text-tertiary">{t("review.noReviewHint")}</p>
        ) : null}
        {hasReview ? <p className="text-xs text-text-tertiary">{t("review.clearHint")}</p> : null}
      </section>
    </div>
  );
}
