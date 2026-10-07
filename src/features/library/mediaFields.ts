/* MISSION-159 — Single source of truth for the per-content-type field policy.

   Two surfaces used to render all four runtime counters (pages, episode
   length, episodes, chapters) for every type, so a novel was asked for an
   episode length and a film for a chapter count. Which counter means something
   for a type was known only to the backend (`domain/progress.rs`,
   `ProgressTemplate::for_content_type`).

   This module is that knowledge on the front end: the counters a type uses,
   the label each one carries, and the state that marks a unit consumed. Every
   table is exhaustive over `ContentType`, so adding a type to
   `contentTypes.ts` stops compiling until its policy is stated here — the same
   way the Rust `match` has no catch-all arm and `every_content_type_has_a_template`
   fails. `mediaFields.test.ts` additionally checks these tables against the
   Rust source, so the two cannot drift apart (the hand-written copy this
   replaces had already drifted: it called a podcast "read", while the backend
   marks podcast episodes watched). */

import type { ContentType } from "./contentTypes";

/** The four optional runtime counters a media can carry. */
export type CounterField = "pages" | "epCount" | "chCount" | "durationMin";

/** The counters a type can use, in display order. `book` counts pages, a
    novel/manga counts chapters, a series counts episodes, a film has a runtime. */
const COUNTER_FIELDS: Record<ContentType, readonly CounterField[]> = {
  book: ["pages"],
  novel: ["chCount"],
  web_novel: ["chCount"],
  manga: ["chCount"],
  manhwa: ["chCount"],
  manhua: ["chCount"],
  comic: ["chCount"],
  anime: ["epCount", "durationMin"],
  tv: ["epCount", "durationMin"],
  podcast: ["epCount", "durationMin"],
  movie: ["durationMin"],
  game: [],
  music: [],
  other: [],
};

/** The counters a content type can use, in display order. */
export function counterFields(contentType: string): readonly CounterField[] {
  return COUNTER_FIELDS[contentType as ContentType] ?? [];
}

/** Whether a counter means anything for a content type. */
export function usesCounter(contentType: string, field: CounterField): boolean {
  return counterFields(contentType).includes(field);
}

/** State that marks a unit consumed, per type (mirrors the Rust template:
    what you *watch* is anime, tv, film and podcast — not the rest). */
const CONSUMING_STATE: Record<ContentType, "read" | "watched"> = {
  book: "read",
  novel: "read",
  web_novel: "read",
  manga: "read",
  manhwa: "read",
  manhua: "read",
  comic: "read",
  anime: "watched",
  tv: "watched",
  movie: "watched",
  podcast: "watched",
  game: "read",
  music: "read",
  other: "read",
};

/** The state a unit ends in when consumed (drives labels and unread filters). */
export function consumingStateFor(contentType: string): "read" | "watched" {
  return CONSUMING_STATE[contentType as ContentType] ?? "read";
}

const COUNTER_LABEL_KEY: Record<CounterField, string> = {
  pages: "library.fieldPages",
  epCount: "library.fieldEpisodes",
  chCount: "library.fieldChapters",
  durationMin: "library.fieldDuration",
};

/** Every counter, for callers that iterate the whole set (the add form clears
    the ones the chosen type cannot use). Derived from the label table above, so
    it cannot fall behind. */
export const COUNTER_FIELD_LIST = Object.keys(COUNTER_LABEL_KEY) as CounterField[];

/** Add-form label for a counter. A film has a runtime, not an episode length. */
export function counterLabelKey(field: CounterField, contentType: string): string {
  if (field === "durationMin" && contentType === "movie") return "library.fieldRuntime";
  return COUNTER_LABEL_KEY[field];
}

const META_LABEL_KEY: Record<CounterField, string> = {
  pages: "detail.metaPages",
  epCount: "detail.metaEpisodes",
  chCount: "detail.metaChapters",
  durationMin: "detail.metaDuration",
};

/** Detail-view label for a counter (the cell appends the unit itself). */
export function counterMetaLabelKey(field: CounterField, contentType: string): string {
  if (field === "durationMin" && contentType === "movie") return "detail.metaRuntime";
  return META_LABEL_KEY[field];
}
