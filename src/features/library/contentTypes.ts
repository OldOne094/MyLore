/* MISSION-145 — Single source of truth for content types.
   Mirrors the Rust `ContentType` enum (`domain/enums.rs`, schema order) so the
   library grouping, filter menus, add-media schema, import mapping and Discover
   selector can never drift apart again (MISSION-109 added game/podcast/music/
   comic and every hand-maintained copy missed them).

   Order is the schema enum order; the last entry is always `other`. */

export const CONTENT_TYPES = [
  "book",
  "novel",
  "web_novel",
  "manga",
  "manhwa",
  "manhua",
  "anime",
  "tv",
  "movie",
  "game",
  "podcast",
  "music",
  "comic",
  "other",
] as const;

export type ContentType = (typeof CONTENT_TYPES)[number];

const NO_TYPES: ReadonlySet<string> = new Set<string>();

/**
 * Content types no *enabled* provider serves (MISSION-145). Used by Discover to
 * explain an empty result ("no providers for this type yet") instead of a bare
 * "no results".
 *
 * Returns an empty set when the provider snapshot has no enabled entries — in
 * that state we cannot conclude a type is unsupported, and flagging all of them
 * would be wrong.
 */
export function unsupportedContentTypes(
  providers: readonly { enabled: boolean; content_types: string[] }[],
): ReadonlySet<string> {
  const enabled = providers.filter((provider) => provider.enabled);
  if (enabled.length === 0) return NO_TYPES;

  const served = new Set<string>();
  for (const provider of enabled) {
    for (const type of provider.content_types) served.add(type);
  }

  const unsupported = new Set<string>();
  for (const type of CONTENT_TYPES) {
    if (!served.has(type)) unsupported.add(type);
  }
  return unsupported;
}
