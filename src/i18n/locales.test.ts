import { describe, expect, it } from "vitest";
import { resources } from "./locales";

type RecordValue = Record<string, unknown>;

function flatten(value: RecordValue, prefix = ""): string[] {
  return Object.entries(value).flatMap(([key, child]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof child === "string" ? [path] : flatten(child as RecordValue, path);
  });
}

describe("translation resources", () => {
  const en = resources.en.translation as unknown as RecordValue;
  const ar = resources.ar.translation as unknown as RecordValue;
  const enKeys = flatten(en).sort();
  const arKeys = flatten(ar).sort();
  const pluralSuffixes = /_(one|other|zero|two|few|many|plural)$/;

  it("non-plural keys match exactly between languages", () => {
    const enNonPlural = enKeys.filter((key) => !pluralSuffixes.test(key)).sort();
    const arNonPlural = arKeys.filter((key) => !pluralSuffixes.test(key)).sort();
    expect(arNonPlural).toEqual(enNonPlural);
  });

  it("covers every English plural form", () => {
    const enPlural = enKeys.filter((key) => pluralSuffixes.test(key));
    for (const key of enPlural) {
      expect(arKeys).toContain(key);
    }
  });

  it("every value is a non-empty string", () => {
    const assertValues = (tree: RecordValue, lang: "en" | "ar") => {
      const visit = (node: RecordValue) => {
        for (const value of Object.values(node)) {
          if (typeof value === "string") {
            expect(value.trim(), `${lang} empty string`).not.toBe("");
          } else {
            visit(value as RecordValue);
          }
        }
      };
      visit(tree);
    };
    assertValues(en, "en");
    assertValues(ar, "ar");
  });
});
