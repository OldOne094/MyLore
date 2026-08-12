import { describe, expect, it } from "vitest";
import { buildLibraryRows, CONTENT_TYPE_ORDER, PUB_STATUS_ORDER } from "./grouping";
import type { MediaListItem } from "./api";

const item = (overrides: Partial<MediaListItem>): MediaListItem => ({
  id: "m-1",
  content_type: "novel",
  title: "Title",
  pub_status: "ongoing",
  release_year: 2024,
  cover_asset_id: null,
  updated_at: "2026-01-01T00:00:00Z",
  ...overrides,
});

const labelFor = (group: string, raw: string) => `${group}:${raw}`;

describe("buildLibraryRows", () => {
  it("returns nothing for an empty list", () => {
    expect(buildLibraryRows([], "none", 2, labelFor)).toEqual([]);
  });

  it("chunks a flat list into grid rows without headers", () => {
    const items = [item({ id: "m-1" }), item({ id: "m-2" }), item({ id: "m-3" })];
    const rows = buildLibraryRows(items, "none", 2, labelFor);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toEqual({ kind: "items", key: "m-1,m-2", items: [items[0], items[1]] });
    expect(rows[1]).toEqual({ kind: "items", key: "m-3", items: [items[2]] });
  });

  it("groups by content type in enum order and emits headers", () => {
    const anime = item({ id: "m-a", content_type: "anime", title: "Anime Title" });
    const book = item({ id: "m-b", content_type: "book", title: "Book Title" });
    const rows = buildLibraryRows([anime, book], "content_type", 2, labelFor);

    expect(rows.map((row) => row.kind)).toEqual(["header", "items", "header", "items"]);
    expect(rows[0]).toEqual({
      kind: "header",
      key: "content_type:book",
      label: "content_type:book",
    });
    expect(rows[1]).toEqual({ kind: "items", key: "m-b", items: [book] });
    expect(rows[2]).toEqual({
      kind: "header",
      key: "content_type:anime",
      label: "content_type:anime",
    });
  });

  it("groups by year descending with unknown last", () => {
    const y2024 = item({ id: "m-1", release_year: 2024 });
    const noYear = item({ id: "m-2", release_year: null });
    const y1999 = item({ id: "m-3", release_year: 1999 });
    const rows = buildLibraryRows([y2024, noYear, y1999], "year", 1, labelFor);

    const headers = rows.filter((row) => row.kind === "header");
    expect(headers).toEqual([
      { kind: "header", key: "year:2024", label: "year:2024" },
      { kind: "header", key: "year:1999", label: "year:1999" },
      { kind: "header", key: "year:\u0000unknown", label: "year:unknown" },
    ]);
  });

  it("orders unknown groups after known ones", () => {
    const known = item({ id: "m-known", pub_status: "ongoing" });
    const unknown = item({ id: "m-unknown", pub_status: "unknown" });
    const rows = buildLibraryRows([unknown, known], "pub_status", 1, labelFor);
    expect(rows.filter((row) => row.kind === "header")[0].label).toBe("pub_status:ongoing");
    expect(rows.filter((row) => row.kind === "header")[1].label).toBe("pub_status:unknown");
  });

  it("exposes the canonical enum orders", () => {
    expect(CONTENT_TYPE_ORDER[0]).toBe("book");
    expect(CONTENT_TYPE_ORDER[CONTENT_TYPE_ORDER.length - 1]).toBe("other");
    expect(PUB_STATUS_ORDER[0]).toBe("announced");
    expect(PUB_STATUS_ORDER[PUB_STATUS_ORDER.length - 1]).toBe("unknown");
  });
});
