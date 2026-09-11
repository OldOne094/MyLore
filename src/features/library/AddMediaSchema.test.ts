import { describe, expect, it } from "vitest";
import { toMediaCreateArgs } from "./api";
import { addMediaSchema } from "./AddMediaSchema";

/* MISSION-038 — Add-media validation contract: required title + valid type,
   empty optionals resolve to undefined, free-text numbers/normalized. */

function parse(input: Record<string, unknown>) {
  const result = addMediaSchema.safeParse(input);
  if (!result.success) {
    throw new Error(`parse failed: ${result.error.issues.map((i) => i.message).join(", ")}`);
  }
  return result.data;
}

describe("addMediaSchema", () => {
  it("accepts a minimal entry and trims the title", () => {
    const data = parse({ title: "  Steins;Gate  ", contentType: "anime" });
    expect(data.title).toBe("Steins;Gate");
    expect(data.genres).toEqual([]);
    expect(data.contentType).toBe("anime");
  });

  it("rejects a blank title", () => {
    const result = addMediaSchema.safeParse({ title: "   ", contentType: "anime" });
    expect(result.success).toBe(false);
  });

  it("rejects an unknown content type", () => {
    const result = addMediaSchema.safeParse({ title: "X", contentType: "fanfic" });
    expect(result.success).toBe(false);
  });

  it("accepts the content types added by MISSION-109", () => {
    // Regression guard (MISSION-145): the add-media picker must not drift
    // behind the domain enum again.
    for (const contentType of ["game", "podcast", "music", "comic"] as const) {
      expect(addMediaSchema.safeParse({ title: "X", contentType }).success).toBe(true);
    }
  });

  it("normalizes an empty numeric field to undefined", () => {
    const data = parse({ title: "X", contentType: "manga", releaseYear: "" });
    expect(data.releaseYear).toBeUndefined();
  });

  it("coerces free-text numbers", () => {
    const data = parse({
      title: "X",
      contentType: "manhwa",
      releaseYear: "1999",
      pages: "240",
    });
    expect(data.releaseYear).toBe(1999);
    expect(data.pages).toBe(240);
  });

  it("rejects a malformed numeric field", () => {
    const result = addMediaSchema.safeParse({
      title: "X",
      contentType: "novel",
      epCount: "four",
    });
    expect(result.success).toBe(false);
  });

  it("rejects an out-of-range year", () => {
    const result = addMediaSchema.safeParse({
      title: "X",
      contentType: "movie",
      releaseYear: "1200",
    });
    expect(result.success).toBe(false);
  });

  it("maps an empty publication status to undefined", () => {
    const data = parse({ title: "X", contentType: "tv", pubStatus: "" });
    expect(data.pubStatus).toBeUndefined();
  });

  it("keeps a chosen publication status", () => {
    const data = parse({ title: "X", contentType: "tv", pubStatus: "ongoing" });
    expect(data.pubStatus).toBe("ongoing");
  });

  it("splits a comma-separated genre list", () => {
    const data = parse({
      title: "X",
      contentType: "book",
      genres: " sci-fi, thriller , mystery",
    });
    expect(data.genres).toEqual(["sci-fi", "thriller", "mystery"]);
  });

  it("strips trailing spaces from optional text", () => {
    const data = parse({ title: "X", contentType: "anime", language: "en " });
    expect(data.language).toBe("en");
  });
});

describe("toMediaCreateArgs", () => {
  it("maps the input shape to the flat IPC arg shape", () => {
    const args = toMediaCreateArgs({
      title: "Steins;Gate",
      contentType: "anime",
      format: "TV",
      pubStatus: "completed",
      synopsis: "A sci-fi thriller.",
      releaseYear: 2011,
      language: "ja",
      country: "JP",
      pages: undefined,
      durationMin: 24,
      epCount: 24,
      chCount: undefined,
      genres: ["sci-fi"],
    });
    expect(args).toEqual({
      title: "Steins;Gate",
      contentType: "anime",
      format: "TV",
      pubStatus: "completed",
      synopsis: "A sci-fi thriller.",
      releaseYear: 2011,
      language: "ja",
      country: "JP",
      pages: null,
      durationMin: 24,
      epCount: 24,
      chCount: null,
      genres: ["sci-fi"],
    });
  });
});
