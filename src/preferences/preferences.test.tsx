import { describe, expect, it, afterEach, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { ToastProvider } from "@/components/ui";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { getPreferencesRepository, parsePreferences } from "@/preferences/repository";
import { DEFAULT_PREFERENCES, type Preferences } from "@/preferences/types";
import { appRoutes } from "@/routes";
import "@/i18n";
import i18n from "@/i18n";

const PREFERENCES_KEY = "mylore.preferences";

function renderSettings() {
  const router = createMemoryRouter(appRoutes, { initialEntries: ["/settings"] });
  return render(
    <ThemeProvider>
      <PreferencesProvider>
        <ToastProvider>
          <RouterProvider router={router} />
        </ToastProvider>
      </PreferencesProvider>
    </ThemeProvider>,
  );
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.removeAttribute("dir");
  document.documentElement.removeAttribute("lang");
});

afterEach(async () => {
  await i18n.changeLanguage("en");
});

describe("preferences repository", () => {
  it("round-trips preferences through the localStorage backend", async () => {
    const repo = getPreferencesRepository();
    const input: Preferences = { theme: "dark", language: "ar" };
    await repo.save(input);
    await expect(repo.load()).resolves.toEqual(input);
    expect(JSON.parse(localStorage.getItem(PREFERENCES_KEY) ?? "{}")).toEqual(input);
  });

  it("resolves null when nothing is stored", async () => {
    const repo = getPreferencesRepository();
    await expect(repo.load()).resolves.toBeNull();
  });

  it("tolerates corrupt or partial storage", () => {
    expect(parsePreferences(null)).toBeNull();
    expect(parsePreferences("garbage")).toBeNull();
    expect(parsePreferences({})).toEqual(DEFAULT_PREFERENCES);
    expect(parsePreferences({ theme: "nope" })).toEqual({
      theme: "system",
      language: "en",
    });
    expect(parsePreferences({ theme: "dark", language: "fr" })).toEqual({
      theme: "dark",
      language: "en",
    });
  });
});

describe("settings page", () => {
  it("renders theme and language sections", async () => {
    renderSettings();
    expect(await screen.findByText("Theme")).toBeInTheDocument();
    expect(screen.getByText("Language")).toBeInTheDocument();
  });

  it("changes the theme and persists it to the preferences store", async () => {
    const user = userEvent.setup();
    renderSettings();
    const themeGroup = await screen.findByRole("group", { name: "Theme" });
    await user.click(within(themeGroup).getByRole("button", { name: "Dark" }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(localStorage.getItem("mylore.theme")).toBe("dark");
    await waitFor(() => {
      expect(JSON.parse(localStorage.getItem(PREFERENCES_KEY) ?? "{}").theme).toBe("dark");
    });
  });

  it("changes the language with RTL and persists it", async () => {
    const user = userEvent.setup();
    renderSettings();
    const languageGroup = await screen.findByRole("group", { name: "Language" });
    await user.click(within(languageGroup).getByRole("button", { name: "ع" }));
    expect(document.documentElement.getAttribute("dir")).toBe("rtl");
    expect(document.documentElement.getAttribute("lang")).toBe("ar");
    await waitFor(() => {
      expect(JSON.parse(localStorage.getItem(PREFERENCES_KEY) ?? "{}").language).toBe("ar");
    });
  });

  it("applies persisted preferences on mount", async () => {
    localStorage.setItem(PREFERENCES_KEY, JSON.stringify({ theme: "dark", language: "ar" }));
    renderSettings();
    await waitFor(() => {
      expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
      expect(document.documentElement.getAttribute("dir")).toBe("rtl");
    });
    expect(await screen.findByRole("link", { name: "المكتبة" })).toBeInTheDocument();
  });
});
