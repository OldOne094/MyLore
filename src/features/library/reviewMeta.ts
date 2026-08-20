/* MISSION-079 — StoryGraph-style review metadata. The fixed mood / pace /
   content-warning vocabularies backed by `mood.*`, `pace.*` and `warning.*`
   i18n keys. Keys must match the Rust domain vocabulary in
   `src-tauri/src/domain/review.rs` — the server rejects anything else. */

export const MOODS = [
  "adventurous",
  "dark",
  "emotional",
  "funny",
  "hopeful",
  "inspiring",
  "informative",
  "lighthearted",
  "mysterious",
  "romantic",
  "sad",
  "tense",
] as const;

export const PACES = ["slow", "medium", "fast"] as const;

export const CONTENT_WARNINGS = [
  "violence",
  "gore",
  "sexual_content",
  "strong_language",
  "self_harm",
  "suicide",
  "drug_use",
  "alcohol",
  "death",
  "abuse",
  "bullying",
  "animal_death",
  "racism",
  "transphobia",
] as const;

export type MoodKey = (typeof MOODS)[number];
export type PaceKey = (typeof PACES)[number];
export type ContentWarningKey = (typeof CONTENT_WARNINGS)[number];
