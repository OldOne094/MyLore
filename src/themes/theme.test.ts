import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import {
  applyTheme,
  createThemeSystem,
  isThemePreference,
  matchSystemTheme,
  readPreference,
  resolveTheme,
  THEME_STORAGE_KEY,
  writePreference,
} from "./theme";

function storage(initial: Record<string, string> = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => void map.set(key, value),
  };
}

describe("isThemePreference", () => {
  it("accepts the three preferences only", () => {
    expect(isThemePreference("light")).toBe(true);
    expect(isThemePreference("dark")).toBe(true);
    expect(isThemePreference("system")).toBe(true);
    expect(isThemePreference("sepia")).toBe(false);
    expect(isThemePreference(null)).toBe(false);
    expect(isThemePreference(undefined)).toBe(false);
  });
});

describe("readPreference / writePreference", () => {
  it("defaults to system when nothing is stored", () => {
    expect(readPreference(storage())).toBe("system");
  });

  it("round-trips an explicit preference", () => {
    const store = storage();
    writePreference("dark", store);
    expect(readPreference(store)).toBe("dark");
  });

  it("ignores unknown stored values", () => {
    expect(readPreference(storage({ [THEME_STORAGE_KEY]: "neon" }))).toBe("system");
  });

  it("degrades gracefully when storage throws", () => {
    const broken = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(readPreference(broken)).toBe("system");
    expect(() => writePreference("dark", broken)).not.toThrow();
  });
});

describe("resolveTheme / applyTheme", () => {
  afterEach(() => {
    document.documentElement.removeAttribute("data-theme");
  });

  it("maps explicit preferences to themselves", () => {
    expect(resolveTheme("light")).toBe("light");
    expect(resolveTheme("dark")).toBe("dark");
  });

  it("resolves system through the media query", () => {
    window.matchMedia = vi.fn().mockReturnValue({ matches: true });
    expect(resolveTheme("system")).toBe("dark");
  });

  it("sets the data-theme attribute on <html>", () => {
    expect(applyTheme("dark")).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });
});

describe("createThemeSystem", () => {
  beforeEach(() => {
    document.documentElement.removeAttribute("data-theme");
  });

  it("boots from the persisted preference", () => {
    const system = createThemeSystem(storage({ [THEME_STORAGE_KEY]: "dark" }));
    expect(system.preference).toBe("dark");
    expect(system.resolved).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("setPreference applies and persists", () => {
    const store = storage();
    const system = createThemeSystem(store);
    expect(system.setPreference("light")).toBe("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(store.getItem(THEME_STORAGE_KEY)).toBe("light");
  });

  it("sync re-resolves a system preference from the media query", () => {
    window.matchMedia = vi.fn().mockReturnValue({ matches: true });
    const system = createThemeSystem(storage());
    expect(system.sync()).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });
});

describe("matchSystemTheme", () => {
  it("returns the OS preference", () => {
    window.matchMedia = vi.fn().mockReturnValue({ matches: false });
    expect(matchSystemTheme()).toBe("light");
  });
});
