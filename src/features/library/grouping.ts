/* MISSION-041 — Library grouping. Pure functions turning a flat media list into
   virtualized rows (group header rows + item rows) for Grid/List/Compact. Group
   order follows the schema enum order (content type, pub status) or release
   year descending; `null` release years land in an "Unknown" bucket last. */

import type { MediaListItem } from "./api";

export type LibraryGroupBy = "none" | "content_type" | "pub_status" | "year";

export const LIBRARY_GROUP_BY: LibraryGroupBy[] = ["none", "content_type", "pub_status", "year"];

export const CONTENT_TYPE_ORDER = [
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

export const PUB_STATUS_ORDER = [
  "announced",
  "ongoing",
  "completed",
  "hiatus",
  "cancelled",
  "unknown",
] as const;

/** A full-width section header row (rendered above each group). */
export interface GroupHeaderRow {
  kind: "header";
  key: string;
  label: string;
}

/** A virtualized row of items: `columns` items in Grid, one in List/Compact. */
export interface GroupItemRow {
  kind: "items";
  key: string;
  items: MediaListItem[];
}

export type LibraryRow = GroupHeaderRow | GroupItemRow;

const UNKNOWN_KEY = "\u0000unknown";

function groupRawKey(groupBy: Exclude<LibraryGroupBy, "none">, item: MediaListItem): string {
  switch (groupBy) {
    case "content_type":
      return item.content_type;
    case "pub_status":
      return item.pub_status;
    case "year":
      return item.release_year == null ? UNKNOWN_KEY : String(item.release_year);
  }
}

function groupLabel(groupBy: LibraryGroupBy, raw: string): string {
  if (raw === UNKNOWN_KEY) return "unknown";
  return groupBy === "year" ? raw : raw;
}

function compareGroups(groupBy: Exclude<LibraryGroupBy, "none">, a: string, b: string): number {
  const aUnknown = a === UNKNOWN_KEY;
  const bUnknown = b === UNKNOWN_KEY;
  if (aUnknown !== bUnknown) return aUnknown ? 1 : -1;
  if (groupBy === "year") return Number(b) - Number(a);
  const order: readonly string[] =
    groupBy === "content_type" ? CONTENT_TYPE_ORDER : PUB_STATUS_ORDER;
  const aIdx = order.indexOf(a);
  const bIdx = order.indexOf(b);
  if (aIdx !== -1 && bIdx !== -1 && aIdx !== bIdx) return aIdx - bIdx;
  return a < b ? -1 : a > b ? 1 : 0;
}

/** Build a flat row model for the virtualizer from a sorted media list.
 *  When `groupBy` is "none" (or the list is empty) no headers are emitted. */
export function buildLibraryRows(
  items: MediaListItem[],
  groupBy: LibraryGroupBy,
  columns: number,
  labelFor: (groupBy: LibraryGroupBy, raw: string) => string,
): LibraryRow[] {
  if (items.length === 0) return [];

  const chunk = (group: MediaListItem[], header?: GroupHeaderRow): LibraryRow[] => {
    const rows: LibraryRow[] = [];
    for (let i = 0; i < group.length; i += columns) {
      rows.push({
        kind: "items",
        key: group
          .slice(i, i + columns)
          .map((item) => item.id)
          .join(","),
        items: group.slice(i, i + columns),
      });
    }
    return header ? [header, ...rows] : rows;
  };

  if (groupBy === "none") {
    return chunk(items);
  }

  const buckets = new Map<string, MediaListItem[]>();
  for (const item of items) {
    const raw = groupRawKey(groupBy, item);
    const bucket = buckets.get(raw);
    if (bucket) {
      bucket.push(item);
    } else {
      buckets.set(raw, [item]);
    }
  }

  const rows: LibraryRow[] = [];
  const keys = [...buckets.keys()].sort((a, b) => compareGroups(groupBy, a, b));
  for (const raw of keys) {
    const label = labelFor(groupBy, groupLabel(groupBy, raw));
    const header: GroupHeaderRow = { kind: "header", key: `${groupBy}:${raw}`, label };
    rows.push(...chunk(buckets.get(raw) ?? [], header));
  }
  return rows;
}
