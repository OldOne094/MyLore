import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { DiscoverPage } from "./DiscoverPage";
import type { ExternalSearchView } from "@/api";
import { listItem } from "@/features/library/testFixtures";

const VIEW: ExternalSearchView = {
  local: [listItem({ id: "m-1", title: "Attack on Titan" })],
  groups: [
    {
      provider: "anilist",
      name: "AniList",
      hits: [
        {
          provider: "anilist",
          provider_id: "21",
          title: "Attack on Titan",
          content_type: "anime",
          release_year: 2013,
          cover_url: null,
          synopsis: null,
          url: null,
          identity: { kind: "in_library", media_id: "m-1", score: 1 },
        },
        {
          provider: "anilist",
          provider_id: "999",
          title: "Berserk",
          content_type: "anime",
          release_year: 1997,
          cover_url: null,
          synopsis: null,
          url: null,
          identity: { kind: "new", media_id: null, score: null },
        },
      ],
    },
  ],
  failures: [{ provider: "tmdb", message: "tmdb is unavailable" }],
};

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={["/discover"]}>
          <Routes>
            <Route path="/discover" element={<DiscoverPage />} />
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

describe("DiscoverPage", () => {
  it("prompts for a query before any search runs", () => {
    renderPage();
    expect(screen.getByText("Search your providers")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("submits a query to search_external and groups hits by provider", async () => {
    vi.mocked(invoke).mockResolvedValue(VIEW);
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "attack");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByRole("heading", { name: /AniList/ })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("search_external", {
      query: "attack",
      content_type: null,
    });
  });

  it("renders local hits and flags in-library external hits", async () => {
    vi.mocked(invoke).mockResolvedValue(VIEW);
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "attack");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByRole("heading", { name: "In your library" })).toBeInTheDocument();
    expect(await screen.findByText("In library")).toBeInTheDocument();
    expect(screen.getByText("New")).toBeInTheDocument();
    const links = screen.getAllByRole("link", { name: "Attack on Titan" });
    expect(links).toHaveLength(2);
    expect(links[0]).toHaveAttribute("href", "/library/m-1");
  });

  it("surfaces per-provider failures", async () => {
    vi.mocked(invoke).mockResolvedValue(VIEW);
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "attack");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByText("tmdb wasn't available")).toBeInTheDocument();
  });

  it("shows a no-results state when nothing matches", async () => {
    vi.mocked(invoke).mockResolvedValue({
      local: [],
      groups: [],
      failures: [],
    } satisfies ExternalSearchView);
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "zzz");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByText("No results")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("search_external", {
      query: "zzz",
      content_type: null,
    });
  });
});
