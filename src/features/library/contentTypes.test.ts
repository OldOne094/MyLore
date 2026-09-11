import { describe, expect, it } from "vitest";
import { CONTENT_TYPES, unsupportedContentTypes } from "./contentTypes";

/* MISSION-145 — the shared content-type source and the Discover unsupported
   computation. */

describe("CONTENT_TYPES", () => {
  it("starts with book and ends with other (schema order)", () => {
    expect(CONTENT_TYPES[0]).toBe("book");
    expect(CONTENT_TYPES[CONTENT_TYPES.length - 1]).toBe("other");
  });

  it("includes the types added by MISSION-109", () => {
    for (const type of ["game", "podcast", "music", "comic"]) {
      expect(CONTENT_TYPES).toContain(type);
    }
  });
});

describe("unsupportedContentTypes", () => {
  const provider = (enabled: boolean, content_types: string[]) => ({ enabled, content_types });

  it("flags types no enabled provider serves", () => {
    const unsupported = unsupportedContentTypes([provider(true, ["anime", "manga"])]);
    expect(unsupported.has("anime")).toBe(false);
    expect(unsupported.has("manga")).toBe(false);
    expect(unsupported.has("podcast")).toBe(true);
    expect(unsupported.has("other")).toBe(true);
  });

  it("ignores disabled providers", () => {
    const unsupported = unsupportedContentTypes([
      provider(true, ["anime"]),
      provider(false, ["podcast"]),
    ]);
    expect(unsupported.has("podcast")).toBe(true);
  });

  it("treats a type as served when any enabled provider declares it", () => {
    const unsupported = unsupportedContentTypes([
      provider(true, ["anime"]),
      provider(true, ["podcast", "music"]),
    ]);
    expect(unsupported.has("podcast")).toBe(false);
    expect(unsupported.has("music")).toBe(false);
  });

  it("concludes nothing when no provider is enabled", () => {
    expect(unsupportedContentTypes([]).size).toBe(0);
    expect(unsupportedContentTypes([provider(false, ["anime"])]).size).toBe(0);
  });
});
