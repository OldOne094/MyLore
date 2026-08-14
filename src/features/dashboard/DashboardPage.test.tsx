import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";
import type { DashboardSummary } from "@/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { DashboardPage } from "./DashboardPage";
import { listItem } from "@/features/library/testFixtures";

const NEXT = {
  percent: 40,
  completed: 2,
  total: 5,
  next_label: "E3",
  next_node_id: "e3",
};

const DONE = {
  percent: 100,
  completed: 5,
  total: 5,
  next_label: null,
  next_node_id: null,
};

function summary(overrides: Partial<DashboardSummary> = {}): DashboardSummary {
  return {
    continue_watching: [listItem({ id: "m-1", title: "Book One", progress: NEXT })],
    recently_completed: [
      listItem({ id: "m-2", title: "Anime Two", content_type: "anime", progress: DONE }),
    ],
    recently_added: [listItem({ id: "m-3", title: "Manga Three", content_type: "manga" })],
    ...overrides,
  };
}

function wrap(response: unknown) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "dashboard_summary") return Promise.resolve(response);
    return Promise.resolve([]);
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={["/dashboard"]}>
          <Routes>
            <Route path="/dashboard" element={<DashboardPage />} />
            <Route path="/search" element={<div>SEARCH_PAGE</div>} />
            <Route path="*" element={<div>FALLBACK</div>} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("DashboardPage", () => {
  it("renders the widget grid with the three list sections", async () => {
    wrap(summary());
    expect(
      await screen.findByRole("heading", { name: "Continue reading / watching" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Recently completed" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Recently added" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Book One" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Anime Two" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Manga Three" })).toBeInTheDocument();
  });

  it("shows a calm empty state per widget when the library is empty", async () => {
    wrap(summary({ continue_watching: [], recently_completed: [], recently_added: [] }));
    expect(await screen.findByText("Nothing in progress right now.")).toBeInTheDocument();
    expect(screen.getByText("Nothing completed yet.")).toBeInTheDocument();
    expect(screen.getByText("Nothing added yet.")).toBeInTheDocument();
  });

  it("opens the add-title dialog from Quick actions", async () => {
    const user = userEvent.setup();
    wrap(summary());
    await user.click(await screen.findByRole("button", { name: "Add title" }));
    expect(await screen.findByRole("heading", { name: "Add a title" })).toBeInTheDocument();
  });

  it("dispatches the quick-capture event", async () => {
    const user = userEvent.setup();
    const dispatch = vi.spyOn(window, "dispatchEvent");
    wrap(summary());
    await user.click(await screen.findByRole("button", { name: "Quick capture" }));
    expect(dispatch).toHaveBeenCalledWith(
      expect.objectContaining({ type: "mylore:open-quick-capture" }),
    );
  });

  it("navigates to search from Quick actions", async () => {
    const user = userEvent.setup();
    wrap(summary());
    await user.click(await screen.findByRole("button", { name: "Search" }));
    expect(await screen.findByText("SEARCH_PAGE")).toBeInTheDocument();
  });

  it("surfaces an error with retry when the summary fails", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("boom"));
    wrap(summary());
    expect(await screen.findByText("Couldn't load your dashboard")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Book One")).toBeInTheDocument();
  });
});
