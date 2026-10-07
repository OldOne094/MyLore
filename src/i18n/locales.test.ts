import { describe, expect, it } from "vitest";
import { LOCALES, resources } from "./locales";

type RecordValue = Record<string, unknown>;

function flatten(value: RecordValue, prefix = ""): string[] {
  return Object.entries(value).flatMap(([key, child]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof child === "string" ? [path] : flatten(child as RecordValue, path);
  });
}

const pluralSuffixes = /_(one|other|zero|two|few|many|plural)$/;

describe("translation resources", () => {
  const en = resources.en.translation as unknown as RecordValue;
  const enKeys = flatten(en).sort();
  const enNonPlural = enKeys.filter((key) => !pluralSuffixes.test(key)).sort();
  const enPlural = enKeys.filter((key) => pluralSuffixes.test(key));
  const languages = Object.keys(resources) as Array<keyof typeof resources>;
  const others = languages.filter((code) => code !== "en");

  it("has one locale descriptor per language, with a direction and a label", () => {
    expect(Object.keys(LOCALES).sort()).toEqual([...languages].sort());
    for (const code of languages) {
      const descriptor = LOCALES[code];
      expect(descriptor.short.trim(), `${code} short label`).not.toBe("");
      expect(["ltr", "rtl"], `${code} direction`).toContain(descriptor.dir);
    }
  });

  it("keeps every English non-plural key in every language", () => {
    for (const code of others) {
      const keys = flatten(resources[code].translation as unknown as RecordValue)
        .filter((key) => !pluralSuffixes.test(key))
        .sort();
      expect(keys, `${code} non-plural keys`).toEqual(enNonPlural);
    }
  });

  it("covers every English plural form in every language", () => {
    for (const code of others) {
      const keys = flatten(resources[code].translation as unknown as RecordValue);
      for (const key of enPlural) {
        expect(keys, `${code} is missing ${key}`).toContain(key);
      }
    }
  });

  it("has no empty string in any language", () => {
    for (const code of languages) {
      const visit = (node: RecordValue) => {
        for (const value of Object.values(node)) {
          if (typeof value === "string") {
            expect(value.trim(), `${code} empty string`).not.toBe("");
          } else {
            visit(value as RecordValue);
          }
        }
      };
      visit(resources[code].translation as unknown as RecordValue);
    }
  });
});
