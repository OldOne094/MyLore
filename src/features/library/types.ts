/* MISSION-038 — Library feature types. Mirrors the Rust domain slugs
   (ContentType / MediaStatus in src-tauri/src/domain/enums.rs) and the flat
   IPC arg shape for `media_create`. */

export type ContentType =
  | "book"
  | "novel"
  | "web_novel"
  | "manga"
  | "manhwa"
  | "manhua"
  | "anime"
  | "tv"
  | "movie"
  | "other";

export type PublicationStatus =
  "announced" | "ongoing" | "completed" | "hiatus" | "cancelled" | "unknown";

/** User-facing shape resolved by the Add form (schema output type). */
export interface AddMediaInput {
  title: string;
  contentType: ContentType;
  format?: string;
  pubStatus?: PublicationStatus;
  synopsis?: string;
  releaseYear?: number;
  language?: string;
  country?: string;
  pages?: number;
  durationMin?: number;
  epCount?: number;
  chCount?: number;
  genres: string[];
}
