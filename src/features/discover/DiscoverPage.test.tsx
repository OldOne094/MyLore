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
import { DiscoverPage } from "./DiscoverPage";
import type { ExternalSearchView, ProviderSettingsView } from "@/api";
import { listItem } from "@/features/library/testFixtures";

/** Providers covering every content type except `other` + `podcast`, so the
    "no provider for this type" notice has a real unsupported type to show. */
const PROVIDERS: ProviderSettingsView[] = [
  {
    provider: "anilist",
    name: "AniList",
    enabled: true,
    requires_key: false,
    has_key: false,
    content_types: [
      "anime",
      "manga",
      "manhwa",
      "novel",
      "web_novel",
      "book",
      "tv",
      "movie",
      "game",
      "music",
      "comic",
    ],
  },
];

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
            <Route
              path="/library/:id"
              element={<div data-testid="library-detail">Library detail</div>}
            />
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

/** Route each command to a fixture; `providers_list` always resolves so the
    unsupported-type computation has data (MISSION-145). */
function mockCommands(map: Record<string, unknown>, providers: ProviderSettingsView[] = PROVIDERS) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "providers_list") return providers;
    if (cmd in map) return map[cmd];
    throw new Error(`unexpected command ${cmd}`);
  });
}

describe("DiscoverPage", () => {
  it("prompts for a query before any search runs", () => {
    mockCommands({});
    renderPage();
    expect(screen.getByText("Search your providers")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("search_external", expect.anything());
  });

  it("submits a query to search_external and groups hits by provider", async () => {
    mockCommands({ search_external: VIEW });
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "attack");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByRole("heading", { name: /AniList/ })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("search_external", {
      query: "attack",
      contentType: null,
    });
  });

  it("renders local hits and flags in-library external hits", async () => {
    mockCommands({ search_external: VIEW });
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
    mockCommands({ search_external: VIEW });
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "attack");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByText("tmdb wasn't available")).toBeInTheDocument();
  });

  it("shows a no-results state when nothing matches", async () => {
    mockCommands({
      search_external: {
        local: [],
        groups: [],
        failures: [],
      } satisfies ExternalSearchView,
    });
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "zzz");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByText("No results")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("search_external", {
      query: "zzz",
      contentType: null,
    });
  });

  it("explains when the selected type has no serving provider", async () => {
    mockCommands({
      search_external: {
        local: [],
        groups: [],
        failures: [],
      } satisfies ExternalSearchView,
    });
    const user = userEvent.setup();
    renderPage();

    // Podcast is the type no fixture provider serves (MISSION-145).
    await user.selectOptions(screen.getByRole("combobox", { name: "All types" }), "podcast");
    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "stuff");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByText("No providers for this type yet")).toBeInTheDocument();
  });

  it("imports a new hit, shows the success toast and navigates to it", async () => {
    mockCommands({
      search_external: VIEW,
      import_provider: {
        media_id: "m-42",
        created: true,
        identity_kind: "new",
        title: "Berserk",
        content_type: "anime",
      },
    });
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "berserk");
    await user.click(screen.getByRole("button", { name: "Search" }));

    const addButton = await screen.findByRole("button", { name: "Add to library" });
    expect(addButton).toBeInTheDocument();
    expect(screen.queryByText("In library")).toBeInTheDocument();

    await user.click(addButton);
    expect(await screen.findByTestId("library-detail")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("import_provider", {
      provider: "anilist",
      providerId: "999",
    });
    expect(await screen.findByText("Added “Berserk” to your library")).toBeInTheDocument();
  });

  it("resolves an already-imported hit to its library page without an add button", async () => {
    mockCommands({ search_external: VIEW });
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "attack");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByText("In library")).toBeInTheDocument();
    const links = screen.getAllByRole("link", { name: "Attack on Titan" });
    expect(links).toHaveLength(2);
    // Only the "new" hit shows an import button; the in-library hit links out.
    expect(screen.getAllByRole("button", { name: "Add to library" })).toHaveLength(1);
  });

  it("imports from the detail dialog through the shared flow (toast + navigate)", async () => {
    // MISSION-146 — the dialog's Import runs the same flow as the row's:
    // provider_get_details fills the dialog, then import + toast + navigate.
    mockCommands({
      search_external: VIEW,
      provider_get_details: {
        provider: "anilist",
        provider_id: "999",
        title_main: "Berserk",
        title_original: null,
        alt_titles: [],
        content_type: "anime",
        format: null,
        pub_status: "ongoing",
        synopsis: "A dark fantasy.",
        start_date: null,
        end_date: null,
        release_year: 1989,
        language: null,
        country: null,
        content_rating: null,
        pages: null,
        duration_min: null,
        ep_count: null,
        ch_count: null,
        cover_url: null,
        banner_url: null,
        url: null,
        people: [],
        genres: [],
        tags: [],
      },
      import_provider: {
        media_id: "m-42",
        created: true,
        identity_kind: "new",
        title: "Berserk",
        content_type: "anime",
      },
    });
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "berserk");
    await user.click(screen.getByRole("button", { name: "Search" }));

    // Open the detail dialog from the "new" hit's title button.
    await user.click(await screen.findByRole("button", { name: "Berserk" }));
    const dialog = await screen.findByRole("dialog");
    // Details loaded (synopsis from provider_get_details).
    expect(await within(dialog).findByText("A dark fantasy.")).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: "Add to library" }));

    expect(invoke).toHaveBeenCalledWith("import_provider", {
      provider: "anilist",
      providerId: "999",
    });
    expect(await screen.findByTestId("library-detail")).toBeInTheDocument();
    expect(await screen.findByText("Added “Berserk” to your library")).toBeInTheDocument();
  });
});
