import { beforeEach, describe, expect, it } from "vitest";
import i18n, {
  applyLanguage,
  browserLanguage,
  initI18n,
  isRtl,
  LANGUAGE_SHORT_LABELS,
  LOCALE_STORAGE_KEY,
  readLanguage,
  setLanguage,
  SUPPORTED_LANGUAGES,
} from "./index";

beforeEach(async () => {
  localStorage.clear();
  document.documentElement.removeAttribute("dir");
  document.documentElement.removeAttribute("lang");
  await i18n.changeLanguage("en");
});

describe("readLanguage", () => {
  it("prefers the persisted choice", () => {
    localStorage.setItem(LOCALE_STORAGE_KEY, "ar");
    expect(readLanguage()).toBe("ar");
    localStorage.setItem(LOCALE_STORAGE_KEY, "tr");
    expect(readLanguage()).toBe("tr");
    localStorage.setItem(LOCALE_STORAGE_KEY, "es");
    expect(readLanguage()).toBe("es");
  });

  it("ignores unknown stored values", () => {
    localStorage.setItem(LOCALE_STORAGE_KEY, "fr");
    expect(readLanguage()).toBe("en");
  });

  it("falls back to the browser language", () => {
    Object.defineProperty(navigator, "language", {
      value: "ar-EG",
      configurable: true,
    });
    expect(browserLanguage()).toBe("ar");
    expect(readLanguage()).toBe("ar");
  });

  it("matches any supported browser language, not only Arabic and English", () => {
    Object.defineProperty(navigator, "language", { value: "tr-TR", configurable: true });
    expect(browserLanguage()).toBe("tr");
    Object.defineProperty(navigator, "language", { value: "es-419", configurable: true });
    expect(browserLanguage()).toBe("es");
  });
});

describe("supported languages", () => {
  it("derives the list and labels from the locale table", () => {
    expect(SUPPORTED_LANGUAGES).toEqual(["en", "ar", "tr", "es"]);
    expect(LANGUAGE_SHORT_LABELS.tr).toBe("TR");
    expect(LANGUAGE_SHORT_LABELS.es).toBe("ES");
  });
});

describe("isRtl", () => {
  it("maps Arabic to RTL and the rest to LTR", () => {
    expect(isRtl("ar")).toBe(true);
    expect(isRtl("en")).toBe(false);
    expect(isRtl("tr")).toBe(false);
    expect(isRtl("es")).toBe(false);
  });
});

describe("applyLanguage", () => {
  it("sets lang and dir attributes on <html>", () => {
    applyLanguage("ar");
    expect(document.documentElement.getAttribute("lang")).toBe("ar");
    expect(document.documentElement.getAttribute("dir")).toBe("rtl");

    for (const code of ["en", "tr", "es"] as const) {
      applyLanguage(code);
      expect(document.documentElement.getAttribute("lang")).toBe(code);
      expect(document.documentElement.getAttribute("dir")).toBe("ltr");
    }
  });
});

describe("setLanguage", () => {
  it("persists, applies RTL and re-renders i18next", async () => {
    await setLanguage("ar");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("ar");
    expect(document.documentElement.getAttribute("dir")).toBe("rtl");
    expect(i18n.resolvedLanguage).toBe("ar");
  });

  it("switches to a Latin-script language and re-renders i18next", async () => {
    await setLanguage("tr");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("tr");
    expect(document.documentElement.getAttribute("dir")).toBe("ltr");
    expect(i18n.resolvedLanguage).toBe("tr");
  });
});

describe("initI18n", () => {
  it("applies the resolved language to the document", () => {
    localStorage.setItem(LOCALE_STORAGE_KEY, "ar");
    void i18n.changeLanguage("ar");
    const language = initI18n();
    expect(language).toBe("ar");
    expect(document.documentElement.getAttribute("lang")).toBe("ar");
  });
});
