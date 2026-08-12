import { beforeEach, describe, expect, it, afterEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

function renderApp(initialEntry = "/") {
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

const EMPTY_LIBRARY = "Your library is empty";
const STATS_HINT = "Time watched, pages read and your ratings distribution.";

beforeEach(() => {
  vi.mocked(invoke).mockResolvedValue([]);
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.removeAttribute("dir");
  document.documentElement.removeAttribute("lang");
  await i18n.changeLanguage("en");
});

describe("App shell", () => {
  it("redirects the root to the first nav section", async () => {
    renderApp("/");
    expect(await screen.findByText(EMPTY_LIBRARY)).toBeInTheDocument();
  });

  it("navigates between sections via the nav rail", async () => {
    const user = userEvent.setup();
    renderApp("/library");
    await user.click(screen.getByRole("link", { name: "Stats" }));
    expect(await screen.findByText(STATS_HINT)).toBeInTheDocument();
  });

  it("highlights the active nav item", async () => {
    renderApp("/library");
    expect(screen.getByRole("link", { name: "Library" })).toHaveClass("bg-accent-soft");
  });

  it("shows the status bar", () => {
    renderApp("/library");
    expect(screen.getByText("0 titles")).toBeInTheDocument();
  });

  it("switches the theme from the top bar", async () => {
    const user = userEvent.setup();
    renderApp("/library");
    await user.click(screen.getByRole("button", { name: "Dark" }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("switches the language to Arabic with RTL", async () => {
    const user = userEvent.setup();
    renderApp("/library");
    await user.click(screen.getByRole("button", { name: "ع" }));
    expect(await screen.findByRole("link", { name: "المكتبة" })).toBeInTheDocument();
    expect(document.documentElement.getAttribute("dir")).toBe("rtl");
    expect(document.documentElement.getAttribute("lang")).toBe("ar");
  });
});
