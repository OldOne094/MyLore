import { describe, expect, it } from "vitest";
import type { ContentNode } from "@/api";
import { consumingStateFor, nodeUnitLabel, unreadUnits } from "./progress";

function unit(
  id: string,
  kind: string,
  position: number,
  state: string | null,
  number?: string,
): ContentNode {
  return {
    id,
    kind,
    position,
    number: number ?? null,
    title: null,
    release_date: null,
    duration_min: null,
    page_count: null,
    synopsis: null,
    is_special: false,
    state,
    children: [],
  };
}

const TREE: ContentNode[] = [
  {
    ...unit("s1", "season", 1, null),
    children: [
      unit("e1", "episode", 1, "watched", "1"),
      unit("e2", "episode", 2, "skipped", "2"),
      unit("e3", "episode", 3, null, "3"),
    ],
  },
  unit("v1", "volume", 2, null),
];

describe("unreadUnits", () => {
  it("returns countable units still to consume, in display order", () => {
    const unread = unreadUnits(TREE, "watched");
    expect(unread.map((n) => n.id)).toEqual(["e2", "e3"]);
  });

  it("keeps skipped nodes as candidates but drops consumed ones", () => {
    const unread = unreadUnits(TREE, "read");
    expect(unread.map((n) => n.id)).toEqual(["e1", "e2", "e3"]);
  });

  it("ignores container kinds (seasons, volumes) entirely", () => {
    const unread = unreadUnits(TREE, "watched");
    expect(unread.some((n) => n.kind === "season" || n.kind === "volume")).toBe(false);
  });
});

describe("consumingStateFor", () => {
  it("watches anime, tv and movies", () => {
    expect(consumingStateFor("anime")).toBe("watched");
    expect(consumingStateFor("tv")).toBe("watched");
    expect(consumingStateFor("movie")).toBe("watched");
  });

  it("reads everything else", () => {
    for (const type of ["manga", "novel", "book", "other"]) {
      expect(consumingStateFor(type)).toBe("read");
    }
  });
});

describe("nodeUnitLabel", () => {
  it("formats episodes and chapters with their number", () => {
    expect(nodeUnitLabel(unit("a", "episode", 3, null, "4"))).toBe("E4");
    expect(nodeUnitLabel(unit("a", "chapter", 3, null, "7"))).toBe("Ch7");
  });

  it("falls back to the display position when unnumbered", () => {
    expect(nodeUnitLabel(unit("a", "episode", 3, null))).toBe("E3");
    expect(nodeUnitLabel(unit("a", "node", 5, null))).toBe("#5");
  });
});
