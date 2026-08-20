import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
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
import { MediaDetailPage } from "./MediaDetailPage";
import type { MediaDetail } from "./api";
import type { ContentNode } from "@/api";

const DETAIL: MediaDetail = {
  id: "m-111",
  content_type: "anime",
  format: "tv",
  title_main: "Steins;Gate",
  title_original: "シュタインズ・ゲート",
  synopsis: "A group of friends accidentally discovers time travel.",
  pub_status: "completed",
  start_date: null,
  end_date: null,
  release_year: 2011,
  language: "ja",
  country: "JP",
  content_rating: null,
  pages: null,
  duration_min: 24,
  ep_count: 24,
  ch_count: null,
  cover_asset_id: null,
  banner_asset_id: null,
  provider: null,
  provider_url: null,
  metadata_refreshed_at: null,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-02T00:00:00Z",
  alt_titles: [{ lang: "", title: "Stein's;Gate" }],
  people: [],
  genres: ["science_fiction", "thriller"],
  tags: [],
  external_ids: [],
  relations: [],
};

function renderPage(id = "m-111") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={[`/library/${id}`]}>
          <Routes>
            <Route path="/library/:id" element={<MediaDetailPage />} />
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

const NODES: ContentNode[] = [
  {
    id: "s1",
    kind: "season",
    position: 1,
    number: "1",
    title: null,
    release_date: null,
    duration_min: null,
    page_count: null,
    synopsis: null,
    is_special: false,
    state: null,
    children: [
      {
        id: "e1",
        kind: "episode",
        position: 1,
        number: "1",
        title: "Time Traveler",
        release_date: null,
        duration_min: 24,
        page_count: null,
        synopsis: null,
        is_special: false,
        state: null,
        children: [],
      },
    ],
  },
];

describe("MediaDetailPage", () => {
  it("renders the hero with title, meta badges and a link back to the library", async () => {
    vi.mocked(invoke).mockResolvedValue(DETAIL);
    renderPage();

    expect(await screen.findByRole("heading", { name: "Steins;Gate" })).toBeInTheDocument();
    expect(screen.getByText("シュタインズ・ゲート")).toBeInTheDocument();
    expect(screen.getByText("Anime")).toBeInTheDocument();
    expect(screen.getAllByText("Completed").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("2011").length).toBeGreaterThanOrEqual(1);
    expect(
      screen.getByText("A group of friends accidentally discovers time travel."),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to library" })).toHaveAttribute(
      "href",
      "/library",
    );
    expect(invoke).toHaveBeenCalledWith("media_get", { id: "m-111" });
  });

  it("defaults to the overview tab and renders the aggregate facts", async () => {
    vi.mocked(invoke).mockResolvedValue(DETAIL);
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    const overview = screen.getByRole("tab", { name: "Overview" });
    expect(overview).toHaveAttribute("aria-selected", "true");
    expect(overview.parentElement).toHaveAttribute("role", "tablist");

    const panel = screen.getByRole("tabpanel");
    expect(within(panel).getByText("Science Fiction")).toBeInTheDocument();
    expect(within(panel).getByText("Thriller")).toBeInTheDocument();
    expect(within(panel).getByText("24")).toBeInTheDocument();
    expect(within(panel).getByText("ja")).toBeInTheDocument();
    expect(within(panel).getByText("JP")).toBeInTheDocument();
  });

  it("switches tabs with the keyboard and shows the tracking status picker", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "media_get") return Promise.resolve(DETAIL);
      if (cmd === "tracking_get") return Promise.resolve(null);
      if (cmd === "review_get") return Promise.resolve(null);
      if (cmd === "media_tags") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    const tab = screen.getByRole("tab", { name: "Tracking" });
    await userEvent.click(tab);
    expect(tab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("group", { name: "Tracking status" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Planned" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("tab", { name: "Review" }));
    expect(screen.getByRole("button", { name: "Save review" })).toBeInTheDocument();
    expect(screen.getByText("No review yet — write one below.")).toBeInTheDocument();
  });

  it("renders the content tree in the Details tab", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "media_get") return Promise.resolve(DETAIL);
      if (cmd === "media_nodes") return Promise.resolve(NODES);
      return Promise.resolve(undefined);
    });
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    await userEvent.click(screen.getByRole("tab", { name: "Details" }));

    expect(
      await screen.findByRole("tree", { name: "Steins;Gate content tree" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: "Season 1" })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: "Episode 1 · Time Traveler" })).toHaveAttribute(
      "aria-level",
      "2",
    );
    expect(invoke).toHaveBeenCalledWith("media_nodes", { id: "m-111" });
  });

  it("shows an error state with retry when loading fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("boom").mockResolvedValueOnce(DETAIL);
    renderPage();

    const retry = await screen.findByRole("button", { name: "Retry" });
    await userEvent.click(retry);

    expect(await screen.findByRole("heading", { name: "Steins;Gate" })).toBeInTheDocument();
  });

  it("shows the not-found state when the id resolves to null", async () => {
    vi.mocked(invoke).mockResolvedValue(null);
    renderPage("m-missing");
    expect(await screen.findByText("Title not found")).toBeInTheDocument();
  });

  it("deletes a title, navigates back and offers an undo toast", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "media_get") return Promise.resolve(DETAIL);
      if (cmd === "media_delete") return Promise.resolve("t-1");
      return Promise.resolve(undefined);
    });
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    await userEvent.click(screen.getByRole("button", { name: "Delete Steins;Gate" }));

    expect(invoke).toHaveBeenCalledWith("media_delete", { id: "m-111" });
    expect(await screen.findByText("1 title moved to trash")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(invoke).toHaveBeenCalledWith("trash_restore", { id: "t-1" });
    expect(await screen.findByText("Restored “Steins;Gate”")).toBeInTheDocument();
  });

  it("shows an error toast when the delete fails", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "media_get") return Promise.resolve(DETAIL);
      if (cmd === "review_get") return Promise.resolve(null);
      if (cmd === "media_delete") return Promise.reject("boom");
      return Promise.resolve(undefined);
    });
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    await userEvent.click(screen.getByRole("button", { name: "Delete Steins;Gate" }));

    expect(await screen.findByText("Couldn't delete the title")).toBeInTheDocument();
  });

  it("refreshes from the provider and shows the diff dialog when fields changed", async () => {
    const PROVIDER_DETAIL: MediaDetail = {
      ...DETAIL,
      provider: "anilist",
      provider_url: "https://anilist.co/anime/9253",
    };
    const ENRICH_VIEW = {
      media_id: "m-111",
      provider: "anilist",
      refreshed_at: "2026-03-01T00:00:00Z",
      changed: true,
      changes: [
        { field: "title_main", before: "Steins;Gate", after: "Steins;Gate 0" },
        { field: "ch_count", before: null, after: "24" },
      ],
    };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "media_get") return Promise.resolve(PROVIDER_DETAIL);
      if (cmd === "media_enrich") return Promise.resolve(ENRICH_VIEW);
      return Promise.resolve(undefined);
    });
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    const refresh = screen.getByRole("button", { name: "Refresh Steins;Gate from anilist" });
    await userEvent.click(refresh);

    expect(invoke).toHaveBeenCalledWith("media_enrich", { media_id: "m-111" });
    expect(await screen.findByText("Metadata refresh")).toBeInTheDocument();
    expect(screen.getByText("Refreshed “Steins;Gate” from anilist")).toBeInTheDocument();
    expect(screen.getByText("Title")).toBeInTheDocument();
    expect(screen.getAllByText("Steins;Gate 0").length).toBeGreaterThanOrEqual(1);
  });

  it("refreshes and reports when nothing changed, without showing the dialog", async () => {
    const PROVIDER_DETAIL: MediaDetail = {
      ...DETAIL,
      provider: "tmdb",
      provider_url: "https://www.themoviedb.org/movie/1",
    };
    const ENRICH_VIEW = {
      media_id: "m-111",
      provider: "tmdb",
      refreshed_at: "2026-03-01T00:00:00Z",
      changed: false,
      changes: [],
    };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "media_get") return Promise.resolve(PROVIDER_DETAIL);
      if (cmd === "media_enrich") return Promise.resolve(ENRICH_VIEW);
      return Promise.resolve(undefined);
    });
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    await userEvent.click(screen.getByRole("button", { name: "Refresh Steins;Gate from tmdb" }));

    expect(await screen.findByText("“Steins;Gate” is already up to date")).toBeInTheDocument();
    expect(screen.queryByText("Metadata refresh")).not.toBeInTheDocument();
  });

  it("shows an error toast when the refresh fails", async () => {
    const PROVIDER_DETAIL: MediaDetail = {
      ...DETAIL,
      provider: "anilist",
      provider_url: "https://anilist.co/anime/9253",
    };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "media_get") return Promise.resolve(PROVIDER_DETAIL);
      if (cmd === "review_get") return Promise.resolve(null);
      if (cmd === "media_enrich") return Promise.reject("boom");
      return Promise.resolve(undefined);
    });
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    await userEvent.click(screen.getByRole("button", { name: "Refresh Steins;Gate from anilist" }));

    expect(await screen.findByText("Couldn't refresh “Steins;Gate”")).toBeInTheDocument();
    expect(screen.queryByText("Metadata refresh")).not.toBeInTheDocument();
  });

  it("hides the refresh button when the title has no provider", async () => {
    vi.mocked(invoke).mockResolvedValue(DETAIL);
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    expect(screen.queryByRole("button", { name: /Refresh .* from .*/ })).not.toBeInTheDocument();
  });

  it("renders mood, pace and content-warning badges and acknowledges the warnings", async () => {
    const REVIEW = {
      media_id: "m-111",
      rating: null,
      review: null,
      short_review: null,
      notes: null,
      favorite: false,
      is_spoiler: false,
      moods: ["dark", "tense"],
      pace: "slow",
      content_warnings: ["violence", "gore"],
      warnings_acknowledged_at: null,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-02T00:00:00Z",
    };
    const ACKED = { ...REVIEW, warnings_acknowledged_at: "2026-03-01T00:00:00Z" };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "media_get") return Promise.resolve(DETAIL);
      if (cmd === "review_get") return Promise.resolve(REVIEW);
      if (cmd === "review_acknowledge_warnings") return Promise.resolve(ACKED);
      return Promise.resolve(undefined);
    });
    renderPage();
    await screen.findByRole("heading", { name: "Steins;Gate" });

    expect(await screen.findByText("Dark")).toBeInTheDocument();
    expect(screen.getByText("Tense")).toBeInTheDocument();
    expect(screen.getByText("Slow")).toBeInTheDocument();
    expect(screen.getByText("Violence")).toBeInTheDocument();
    expect(screen.getByText("Gore")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /Acknowledge content warnings for/ }));
    expect(invoke).toHaveBeenCalledWith("review_acknowledge_warnings", {
      media_id: "m-111",
    });
    expect(await screen.findByText("Content warnings acknowledged")).toBeInTheDocument();
    expect(await screen.findByText("Acknowledged")).toBeInTheDocument();
  });
});
