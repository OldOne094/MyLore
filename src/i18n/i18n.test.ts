import { beforeEach, describe, expect, it } from "vitest";
import i18n, {
  applyLanguage,
  browserLanguage,
  initI18n,
  isRtl,
  LOCALE_STORAGE_KEY,
  readLanguage,
  setLanguage,
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
});

describe("isRtl", () => {
  it("maps Arabic to RTL and English to LTR", () => {
    expect(isRtl("ar")).toBe(true);
    expect(isRtl("en")).toBe(false);
  });
});

describe("applyLanguage", () => {
  it("sets lang and dir attributes on <html>", () => {
    applyLanguage("ar");
    expect(document.documentElement.getAttribute("lang")).toBe("ar");
    expect(document.documentElement.getAttribute("dir")).toBe("rtl");

    applyLanguage("en");
    expect(document.documentElement.getAttribute("lang")).toBe("en");
    expect(document.documentElement.getAttribute("dir")).toBe("ltr");
  });
});

describe("setLanguage", () => {
  it("persists, applies RTL and re-renders i18next", async () => {
    await setLanguage("ar");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("ar");
    expect(document.documentElement.getAttribute("dir")).toBe("rtl");
    expect(i18n.resolvedLanguage).toBe("ar");
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
