import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => `asset://${encodeURIComponent(path)}`),
}));

import { invoke } from "@tauri-apps/api/core";
import { LibraryPage } from "./LibraryPage";
import type { MediaFacets } from "./api";
import { NO_PROGRESS } from "./testFixtures";

const TITLES = [
  {
    id: "m-111",
    content_type: "anime",
    title: "Steins;Gate",
    pub_status: "completed",
    release_year: 2011,
    cover_asset_id: null,
    updated_at: "2026-01-01T00:00:00Z",
    favorite: true,
    progress: NO_PROGRESS,
  },
  {
    id: "m-222",
    content_type: "novel",
    title: "Sword of the Dawn",
    pub_status: "ongoing",
    release_year: 2026,
    cover_asset_id: null,
    updated_at: "2026-01-02T00:00:00Z",
    favorite: false,
    progress: NO_PROGRESS,
  },
];

const FACETS: MediaFacets = {
  formats: ["light_novel", "webtoon"],
  genres: [{ id: "fantasy", name: "Fantasy" }],
  tags: [{ id: "isekai", name: "Isekai" }],
  years: [2026, 2011],
};

function mockLibrary(items: unknown, facets: MediaFacets = FACETS) {
  vi.mocked(invoke).mockImplementation((command: string) => {
    if (command === "media_facets") return Promise.resolve(facets);
    if (command === "media_list") return Promise.resolve(items);
    return Promise.resolve(null);
  });
}

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter>
          <LibraryPage />
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

/** Cards and rows are links to the detail page (MISSION-042). */
function libraryCards() {
  return screen.getAllByRole("link", { name: /^Steins;Gate$|^Sword of the Dawn$|^Title \d+$/ });
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("LibraryPage", () => {
  it("renders the empty state when there are no titles", async () => {
    mockLibrary([]);
    renderPage();
    expect(await screen.findByText("Your library is empty")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add title" })).toBeInTheDocument();
  });

  it("renders a grid of cards for existing titles", async () => {
    mockLibrary(TITLES);
    renderPage();

    expect(await screen.findByText("Steins;Gate")).toBeInTheDocument();
    expect(screen.getByText("Sword of the Dawn")).toBeInTheDocument();
    expect(libraryCards()).toHaveLength(2);
    expect(screen.getByText("Anime")).toBeInTheDocument();
    expect(screen.getByText("Novel")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("Ongoing")).toBeInTheDocument();
    expect(screen.queryByText("Your library is empty")).not.toBeInTheDocument();
  });

  it("requests the library listing with default args", async () => {
    mockLibrary([]);
    renderPage();
    await screen.findByText("Your library is empty");

    expect(invoke).toHaveBeenCalledWith(
      "media_list",
      expect.objectContaining({ sort: "title", ascending: true, content_type: null }),
    );
  });

  it("shows a retry action when loading fails", async () => {
    vi.mocked(invoke)
      .mockImplementationOnce((command: string) =>
        command === "media_facets" ? Promise.resolve(FACETS) : Promise.reject("boom"),
      )
      .mockImplementation((command: string) =>
        command === "media_facets" ? Promise.resolve(FACETS) : Promise.resolve(TITLES),
      );
    renderPage();

    const retry = await screen.findByRole("button", { name: "Retry" });
    await userEvent.click(retry);

    await waitFor(() => expect(screen.getByText("Steins;Gate")).toBeInTheDocument());
  });

  it("exposes the view switcher and defaults to the grid", async () => {
    mockLibrary(TITLES);
    renderPage();
    await screen.findByText("Steins;Gate");

    expect(screen.getByRole("group", { name: "Library view" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Grid view" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("switches between List and Compact views", async () => {
    mockLibrary(TITLES);
    renderPage();
    await screen.findByText("Steins;Gate");

    await userEvent.click(screen.getByRole("button", { name: "List view" }));
    expect(screen.getByRole("button", { name: "List view" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "Grid view" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(libraryCards()).toHaveLength(2);

    await userEvent.click(screen.getByRole("button", { name: "Compact list" }));
    expect(screen.getByRole("button", { name: "Compact list" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(libraryCards()).toHaveLength(2);
    expect(screen.getByText("Steins;Gate")).toBeInTheDocument();
  });

  it("flags favorites with a heart in the grid, list and compact views (MISSION-075)", async () => {
    mockLibrary(TITLES);
    renderPage();
    await screen.findByText("Steins;Gate");

    expect(screen.getAllByRole("img", { name: "Favorite" })).toHaveLength(1);

    await userEvent.click(screen.getByRole("button", { name: "List view" }));
    expect(screen.getAllByRole("img", { name: "Favorite" })).toHaveLength(1);

    await userEvent.click(screen.getByRole("button", { name: "Compact list" }));
    expect(screen.getAllByRole("img", { name: "Favorite" })).toHaveLength(1);
  });

  it("windows large libraries instead of rendering every row", async () => {
    const many = Array.from({ length: 300 }, (_, index) => ({
      id: `m-${index}`,
      content_type: "manga",
      title: `Title ${index}`,
      pub_status: "ongoing",
      release_year: 2020,
      cover_asset_id: null,
      updated_at: "2026-01-01T00:00:00Z",
      favorite: false,
      progress: NO_PROGRESS,
    }));
    mockLibrary(many);
    renderPage();
    expect(await screen.findByText("Title 0")).toBeInTheDocument();

    const rendered = libraryCards().length;
    expect(rendered).toBeGreaterThan(0);
    expect(rendered).toBeLessThan(300);
  });

  it("filters the library by content type through media_list", async () => {
    mockLibrary(TITLES);
    renderPage();
    await screen.findByText("Steins;Gate");

    await userEvent.click(screen.getByRole("button", { name: "Filter" }));
    await userEvent.click(await screen.findByRole("button", { name: "Anime" }));

    expect(invoke).toHaveBeenCalledWith(
      "media_list",
      expect.objectContaining({ content_type: "anime" }),
    );
  });

  it("sorts by last updated through media_list", async () => {
    mockLibrary(TITLES);
    renderPage();
    await screen.findByText("Steins;Gate");

    await userEvent.click(screen.getByRole("button", { name: "Sort" }));
    await userEvent.click(await screen.findByRole("button", { name: "Last updated" }));

    expect(invoke).toHaveBeenCalledWith(
      "media_list",
      expect.objectContaining({ sort: "updated_at", ascending: true }),
    );
  });

  it("groups the grid by content type with section headers", async () => {
    mockLibrary(TITLES);
    renderPage();
    await screen.findByText("Steins;Gate");

    await userEvent.click(screen.getByRole("button", { name: "Group by" }));
    await userEvent.click(await screen.findByRole("button", { name: "Content type" }));

    expect(screen.getAllByText("Anime").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("Novel").length).toBeGreaterThanOrEqual(2);
    expect(libraryCards()).toHaveLength(2);
  });

  it("shows a no-results state when filters match nothing", async () => {
    vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
      if (command === "media_facets") return Promise.resolve(FACETS);
      if (command === "media_list") {
        const content_type = (args as { content_type?: string } | undefined)?.content_type;
        return Promise.resolve(content_type === "anime" ? [] : TITLES);
      }
      return Promise.resolve(null);
    });
    renderPage();
    await screen.findByText("Steins;Gate");

    await userEvent.click(screen.getByRole("button", { name: "Filter" }));
    await userEvent.click(await screen.findByRole("button", { name: "Anime" }));

    expect(await screen.findByText("No matching titles")).toBeInTheDocument();
    expect(screen.queryByText("Steins;Gate")).not.toBeInTheDocument();
  });

  it("batch-resolves covers for titles with cover assets (MISSION-062)", async () => {
    const withCovers = TITLES.map((item, index) => ({
      ...item,
      cover_asset_id: `a-${index + 1}`,
    }));
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "media_facets") return Promise.resolve(FACETS);
      if (command === "media_list") return Promise.resolve(withCovers);
      if (command === "assets_resolve") {
        return Promise.resolve([
          {
            id: "a-1",
            kind: "cover",
            status: "cached",
            local_path: "C:/appdata/images/a-1.jpg",
            remote_url: null,
            mime_type: "image/jpeg",
          },
        ]);
      }
      return Promise.resolve(null);
    });
    renderPage();
    await screen.findByText("Steins;Gate");

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("assets_resolve", { asset_ids: ["a-1", "a-2"] });
    });
    expect(await screen.findByRole("img", { name: "Steins;Gate" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Steins;Gate" })).toHaveAttribute(
      "src",
      "asset://C%3A%2Fappdata%2Fimages%2Fa-1.jpg",
    );
  });

  it("does not call assets_resolve when no title has a cover asset", async () => {
    mockLibrary(TITLES);
    renderPage();
    await screen.findByText("Steins;Gate");
    expect(invoke).not.toHaveBeenCalledWith("assets_resolve", expect.objectContaining({}));
  });
});
