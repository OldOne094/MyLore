/* MISSION-117 — "Add a work to my shelf" form schema (Zod). Messages are i18n
   keys (`validation.*`), translated by the dialog before rendering, exactly as
   the add-media form does. The schema's output is the input to
   `reading_group_set_shelf` once the work key has been resolved. */

import { z } from "zod";
import { CONTENT_TYPES } from "@/features/library/contentTypes";

/** Shelf statuses — mirrors the Rust `CoreStatus` storage strings. */
export const SHELF_STATUS_VALUES = [
  "planned",
  "in_progress",
  "completed",
  "on_hold",
  "dropped",
  "repeat",
  "wishlist",
] as const;

const toUndefined = (value: unknown) => (value === undefined || value === "" ? undefined : value);

/** Progress is free-form (pages, chapters, episodes); blank means "not started". */
const toProgress = (value: unknown) => {
  if (value === undefined || value === "") return 0;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : Number.NaN;
};

export const addWorkSchema = z.object({
  title: z.string().trim().min(1, "validation.required").max(300, "validation.tooLong"),
  author: z.preprocess(toUndefined, z.string().trim().max(200, "validation.tooLong").optional()),
  year: z.preprocess(
    toUndefined,
    z.coerce
      .number()
      .int()
      .min(1500, "validation.yearRange")
      .max(3000, "validation.yearRange")
      .optional(),
  ),
  contentType: z.enum(CONTENT_TYPES, { message: "validation.invalid" }),
  status: z.enum(SHELF_STATUS_VALUES, { message: "validation.invalid" }),
  progress: z.preprocess(
    toProgress,
    z.coerce
      .number()
      .int("validation.countRange")
      .min(0, "validation.countRange")
      .max(1_000_000, "validation.countRange"),
  ),
});

export type AddWorkFormValues = z.infer<typeof addWorkSchema>;
export type AddWorkFormInput = z.input<typeof addWorkSchema>;
