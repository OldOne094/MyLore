import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { CONTENT_TYPES } from "./contentTypes";
import {
  consumingStateFor,
  counterFields,
  counterLabelKey,
  counterMetaLabelKey,
  usesCounter,
} from "./mediaFields";

/* MISSION-159 — the per-content-type field policy, and the guard that keeps it
   honest.

   The policy has to exist on both sides: the backend decides progress (it must —
   the engine is pure Rust), while a form has to choose its inputs
   **synchronously**, because a fetch would leave the fields blank on first
   paint. So rather than hope the copies stay in step, this suite reads
   `domain/progress.rs` and asserts the front-end tables agree with the Rust
   template — the same trick `applied_migrations_are_never_edited` uses: the
   source is the fixture. The hand-written copy this replaces had already
   drifted (it called a podcast "read"; the backend marks its episodes watched),
   which is exactly what these assertions pin. */

interface RustTemplate {
  unitKind: string;
  weight: string;
  consumingState: string;
}

/** CamelCase Rust variant name → the snake_case content type code. */
function camelToSnake(name: string): string {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
}

/** Parse `ProgressTemplate::for_content_type` out of the Rust source. */
function rustTemplates(): Map<string, RustTemplate> {
  const path = join(process.cwd(), "src-tauri", "src", "domain", "progress.rs");
  const source = readFileSync(path, "utf8");

  const start = source.indexOf("pub fn for_content_type");
  if (start === -1) throw new Error("for_content_type is gone from domain/progress.rs");
  const end = source.indexOf("/// A UI-ready label", start);
  const block = source.slice(start, end === -1 ? source.length : end);

  const armPattern =
    /([A-Z][A-Za-z]*(?:\s*\|\s*[A-Z][A-Za-z]*)*)\s*=>\s*ProgressTemplate\s*\{([^}]*)\}/g;
  const templates = new Map<string, RustTemplate>();

  for (const arm of block.matchAll(armPattern)) {
    const body = arm[2];
    const unitKind = /unit_kind:\s*NodeKind::(\w+)/.exec(body)?.[1];
    const weight = /weight:\s*UnitWeight::(\w+)/.exec(body)?.[1];
    const consumingState = /consuming_state:\s*NodeProgressState::(\w+)/.exec(body)?.[1];
    if (!unitKind || !weight || !consumingState) {
      throw new Error(`the template for ${arm[1]} no longer parses — update this guard`);
    }
    for (const variant of arm[1].split("|")) {
      templates.set(camelToSnake(variant.trim()), { unitKind, weight, consumingState });
    }
  }
  return templates;
}

describe("media field policy", () => {
  it("asks a book for pages", () => {
    expect(counterFields("book")).toEqual(["pages"]);
  });

  it("asks a novel or a comic for chapters, never pages", () => {
    for (const type of ["novel", "web_novel", "manga", "manhwa", "manhua", "comic"]) {
      expect(counterFields(type)).toEqual(["chCount"]);
    }
  });

  it("asks a series or podcast for episodes and episode length", () => {
    for (const type of ["anime", "tv", "podcast"]) {
      expect(counterFields(type)).toEqual(["epCount", "durationMin"]);
      expect(usesCounter(type, "pages")).toBe(false);
      expect(usesCounter(type, "chCount")).toBe(false);
    }
  });

  it("asks a film for a runtime only", () => {
    expect(counterFields("movie")).toEqual(["durationMin"]);
    expect(usesCounter("movie", "pages")).toBe(false);
    expect(usesCounter("movie", "epCount")).toBe(false);
    expect(usesCounter("movie", "chCount")).toBe(false);
  });

  it("asks game, music and other for no counter at all", () => {
    for (const type of ["game", "music", "other"]) {
      expect(counterFields(type)).toEqual([]);
    }
  });

  it("calls a film's duration a runtime", () => {
    expect(counterLabelKey("durationMin", "movie")).toBe("library.fieldRuntime");
    expect(counterLabelKey("durationMin", "anime")).toBe("library.fieldDuration");
    expect(counterMetaLabelKey("durationMin", "movie")).toBe("detail.metaRuntime");
    expect(counterMetaLabelKey("durationMin", "tv")).toBe("detail.metaDuration");
  });

  it("carries no policy for an unknown type", () => {
    expect(counterFields("fanfic")).toEqual([]);
    expect(consumingStateFor("fanfic")).toBe("read");
  });
});

describe("policy agrees with the Rust progress template", () => {
  const templates = rustTemplates();

  it("covers every content type on both sides", () => {
    expect([...templates.keys()].sort()).toEqual([...CONTENT_TYPES].sort());
  });

  it("counts chapters exactly where the form asks for chapters", () => {
    for (const type of CONTENT_TYPES) {
      const rust = templates.get(type)!;
      const rustCountsChapters = rust.unitKind === "Chapter" && rust.weight === "Count";
      expect(usesCounter(type, "chCount")).toBe(rustCountsChapters);
    }
  });

  it("asks for pages exactly where a page is the unit", () => {
    for (const type of CONTENT_TYPES) {
      expect(usesCounter(type, "pages")).toBe(templates.get(type)!.weight === "Pages");
    }
  });

  it("asks for episodes wherever an episode is the unit — a film is one unit, so it takes a runtime", () => {
    for (const type of CONTENT_TYPES) {
      if (type === "movie") continue;
      expect(usesCounter(type, "epCount")).toBe(templates.get(type)!.unitKind === "Episode");
    }
    expect(usesCounter("movie", "epCount")).toBe(false);
  });

  it("offers no counter where the unit is a free node", () => {
    for (const type of CONTENT_TYPES) {
      if (templates.get(type)!.unitKind !== "Node") continue;
      expect(counterFields(type)).toEqual([]);
    }
  });

  it("consumes with the same state the backend records", () => {
    for (const type of CONTENT_TYPES) {
      expect(consumingStateFor(type)).toBe(templates.get(type)!.consumingState.toLowerCase());
    }
  });
});
