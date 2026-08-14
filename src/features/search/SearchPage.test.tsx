import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { SearchPage } from "./SearchPage";
import type { MediaListItem } from "@/features/library/api";
import { NO_PROGRESS } from "@/features/library/testFixtures";

const ROWS: MediaListItem[] = [
  {
    id: "m-1",
    content_type: "anime",
    title: "Steins;Gate",
    pub_status: "completed",
    release_year: 2011,
    cover_asset_id: null,
    updated_at: "2026-01-01T00:00:00Z",
    progress: NO_PROGRESS,
  },
  {
    id: "m-2",
    content_type: "novel",
    title: "Sword of the Dawn",
    pub_status: "ongoing",
    release_year: 2026,
    cover_asset_id: null,
    updated_at: "2026-01-02T00:00:00Z",
    progress: NO_PROGRESS,
  },
];

function renderPage(query = "steins") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={[`/search?q=${encodeURIComponent(query)}`]}>
          <Routes>
            <Route path="/search" element={<SearchPage />} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("SearchPage", () => {
  it("prompts for a query when none is present", () => {
    renderPage("");
    expect(screen.getByText("Search your library")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("searches the FTS backend for the URL query and lists rows", async () => {
    vi.mocked(invoke).mockResolvedValue([ROWS[0]]);
    renderPage("steins");

    expect(await screen.findByRole("link", { name: "Steins;Gate" })).toHaveAttribute(
      "href",
      "/library/m-1",
    );
    expect(invoke).toHaveBeenCalledWith("media_search", { query: "steins" });
    expect(screen.getByText("1 result for “steins”")).toBeInTheDocument();
  });

  it("shows a no-results state when the backend returns nothing", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    renderPage("zzz");

    expect(await screen.findByText("No matches")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("media_search", { query: "zzz" });
  });

  it("renders every hit as a row", async () => {
    vi.mocked(invoke).mockResolvedValue(ROWS);
    renderPage("dawn");

    expect(await screen.findByRole("link", { name: "Steins;Gate" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sword of the Dawn" })).toHaveAttribute(
      "href",
      "/library/m-2",
    );
  });
});
