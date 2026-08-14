import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { ToastProvider } from "@/components/ui";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { appRoutes } from "@/routes";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

function renderApp(initialEntry = "/library") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(appRoutes, { initialEntries: [initialEntry] });
  return render(
    <QueryClientProvider client={client}>
      <ThemeProvider>
        <PreferencesProvider>
          <ToastProvider>
            <RouterProvider router={router} />
          </ToastProvider>
        </PreferencesProvider>
      </ThemeProvider>
    </QueryClientProvider>,
  );
}

function openPalette() {
  fireEvent.keyDown(window, { key: "k", ctrlKey: true });
}

const LIBRARY_HINT = "Your tracked titles appear here as you add them.";
const SETTINGS_HINT = "Light, dark, or follow the system appearance.";

beforeEach(() => {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "dashboard_summary") {
      return Promise.resolve({ continue_watching: [], recently_completed: [], recently_added: [] });
    }
    return Promise.resolve([]);
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.removeAttribute("dir");
  document.documentElement.removeAttribute("lang");
  await i18n.changeLanguage("en");
});

describe("command palette", () => {
  it("opens with Ctrl+K and lists navigation + theme commands", async () => {
    renderApp();
    openPalette();
    const listbox = await screen.findByRole("listbox");
    expect(screen.getByRole("combobox")).toBeInTheDocument();
    expect(within(listbox).getByText("Navigation")).toBeInTheDocument();
    expect(within(listbox).getByText("Library")).toBeInTheDocument();
    expect(within(listbox).getByText("Actions")).toBeInTheDocument();
    expect(within(listbox).getByText("Dark")).toBeInTheDocument();
  });

  it("filters commands as you type", async () => {
    renderApp();
    openPalette();
    const input = await screen.findByRole("combobox");
    fireEvent.change(input, { target: { value: "settings" } });
    const listbox = screen.getByRole("listbox");
    expect(within(listbox).getByText("Settings")).toBeInTheDocument();
    expect(within(listbox).queryByText("Library")).not.toBeInTheDocument();
  });

  it("navigates with arrow keys + Enter", async () => {
    renderApp("/dashboard");
    openPalette();
    const input = await screen.findByRole("combobox");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText("Your library is empty")).toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("runs a theme command that persists the preference", async () => {
    renderApp();
    openPalette();
    const input = await screen.findByRole("combobox");
    fireEvent.change(input, { target: { value: "dark" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
      expect(localStorage.getItem("mylore.theme")).toBe("dark");
    });
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("navigating to settings renders the real page", async () => {
    renderApp();
    openPalette();
    const input = await screen.findByRole("combobox");
    fireEvent.change(input, { target: { value: "settings" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText(SETTINGS_HINT)).toBeInTheDocument();
    expect(screen.queryByText(LIBRARY_HINT)).not.toBeInTheDocument();
  });
});
