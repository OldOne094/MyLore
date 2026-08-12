/* MISSION-038 — Add-media form schema (Zod). Field messages are i18n keys
   (`validation.*`); the dialog translates them before rendering. Numeric and
   optional fields arrive from RHF as raw strings, so empty values are
   normalized to `undefined` before validation, and the `genres` text field is
   split into a tag list. The schema output type doubles as the IPC input
   shape (`AddMediaInput`). */

import { z } from "zod";

export const CONTENT_TYPE_VALUES = [
  "anime",
  "manga",
  "manhwa",
  "manhua",
  "novel",
  "web_novel",
  "book",
  "tv",
  "movie",
  "other",
] as const;

export const PUBLICATION_STATUS_VALUES = [
  "announced",
  "ongoing",
  "completed",
  "hiatus",
  "cancelled",
  "unknown",
] as const;

const toUndefined = (value: unknown) => (value === undefined || value === "" ? undefined : value);

const optionalText = (max: number) =>
  z.preprocess(toUndefined, z.string().trim().max(max, "validation.tooLong").optional());

const optionalCount = (max: number) =>
  z.preprocess(
    toUndefined,
    z.coerce.number().int().min(0).max(max, "validation.countRange").optional(),
  );

const optionalYear = z.preprocess(
  toUndefined,
  z.coerce
    .number()
    .int()
    .min(1500, "validation.yearRange")
    .max(3000, "validation.yearRange")
    .optional(),
);

export const addMediaSchema = z.object({
  title: z.string().trim().min(1, "validation.required").max(300, "validation.tooLong"),
  contentType: z.enum(CONTENT_TYPE_VALUES, { message: "validation.invalid" }),
  format: optionalText(100),
  pubStatus: z.preprocess(
    toUndefined,
    z.enum(PUBLICATION_STATUS_VALUES, { message: "validation.invalid" }).optional(),
  ),
  synopsis: optionalText(4000),
  releaseYear: optionalYear,
  language: optionalText(3),
  country: optionalText(10),
  pages: optionalCount(1_000_000),
  durationMin: optionalCount(1_000_000),
  epCount: optionalCount(10_000),
  chCount: optionalCount(10_000),
  genres: z
    .string()
    .trim()
    .optional()
    .transform((value) =>
      value === undefined
        ? []
        : value
            .split(",")
            .map((genre) => genre.trim())
            .filter(Boolean),
    ),
});

/** Messages explicitly set in the schema above; anything else uses a generic
 *  "invalid value" message so unexpected codes stay localized. */
const SCHEMA_KEYS = new Set([
  "validation.required",
  "validation.tooLong",
  "validation.invalid",
  "validation.countRange",
  "validation.yearRange",
]);

export function mapIssuesToKeys(issue: { code: string; message?: string }): { message: string } {
  if (issue.message && SCHEMA_KEYS.has(issue.message)) return { message: issue.message };
  return { message: "validation.invalid" };
}

export type AddMediaFormValues = z.infer<typeof addMediaSchema>;

/** Raw values RHF manages before parsing (numeric/optional fields are strings,
 *  `genres` is a comma-separated string). */
export type AddMediaFormInput = z.input<typeof addMediaSchema>;
