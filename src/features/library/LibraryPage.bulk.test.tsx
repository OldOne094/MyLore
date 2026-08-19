import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { LibraryPage } from "./LibraryPage";
import type { MediaFacets } from "./api";
import { NO_PROGRESS } from "./testFixtures";

/* MISSION-045 — Bulk-select mode + action bar. Covers entering select mode,
   select-all/clear, and the status / tag / delete / list actions wiring through
   the bulk IPC commands. MISSION-078 adds the filtered-selection scope (apply
   to all matching) and the per-item change summary toasts. */

const TITLES = [
  {
    id: "m-111",
    content_type: "anime",
    title: "Steins;Gate",
    pub_status: "completed",
    release_year: 2011,
    cover_asset_id: null,
    updated_at: "2026-01-01T00:00:00Z",
    favorite: false,
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

const FULL_SUMMARY = { total: 2, succeeded: 2, failed: 0, failures: [] };

function mockLibrary(items: unknown = TITLES) {
  vi.mocked(invoke).mockImplementation((command: string) => {
    if (command === "media_facets") return Promise.resolve(FACETS);
    if (command === "media_list") return Promise.resolve(items);
    if (command === "tracking_bulk_set_status") return Promise.resolve(FULL_SUMMARY);
    if (command === "media_bulk_add_tag") return Promise.resolve(FULL_SUMMARY);
    if (command === "media_bulk_delete") {
      return Promise.resolve({
        summary: FULL_SUMMARY,
        trash_ids: ["t-1", "t-2"],
      });
    }
    if (command === "collection_list") {
      return Promise.resolve([{ id: "c-1", name: "Reading Now" }]);
    }
    if (command === "collection_bulk_add") {
      return Promise.resolve({ total: 1, succeeded: 1, failed: 0, failures: [] });
    }
    if (command === "trash_restore") return Promise.resolve(undefined);
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

/** The bottom action bar (appears once something is selected). */
function actionBar() {
  return screen.getByRole("toolbar", { name: "Select" });
}

async function enterSelectMode() {
  await screen.findByText("Steins;Gate");
  await userEvent.click(screen.getByRole("button", { name: "Select" }));
  expect(screen.getByRole("button", { name: "Select all" })).toBeInTheDocument();
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("LibraryPage bulk select", () => {
  it("enters select mode and toggles individual cards", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();

    expect(screen.queryByRole("button", { name: "Add title" })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Steins;Gate" }));
    expect(screen.getByRole("button", { name: "Steins;Gate" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(actionBar()).toBeInTheDocument();
    expect(screen.getAllByText("1 title selected").length).toBeGreaterThan(0);

    await userEvent.click(screen.getByRole("button", { name: "Steins;Gate" }));
    expect(screen.getByRole("button", { name: "Steins;Gate" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(screen.queryByRole("toolbar", { name: "Select" })).not.toBeInTheDocument();
  });

  it("selects and clears everything in one step", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Select all" }));
    expect(screen.getByRole("button", { name: "Steins;Gate" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "Sword of the Dawn" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(actionBar()).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear selection" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(screen.queryByRole("toolbar", { name: "Select" })).not.toBeInTheDocument();
  });

  it("exits select mode via the toolbar button", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();
    await userEvent.click(screen.getByRole("button", { name: "Steins;Gate" }));
    expect(actionBar()).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Exit selection" }));
    expect(screen.queryByRole("toolbar", { name: "Select" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Select" })).toBeInTheDocument();
  });

  it("bulk sets the tracking status for every selected id", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Select all" }));
    await userEvent.click(within(actionBar()).getByRole("button", { name: "Status" }));
    await userEvent.click(await screen.findByRole("button", { name: "In progress" }));

    expect(invoke).toHaveBeenCalledWith("tracking_bulk_set_status", {
      ids: ["m-111", "m-222"],
      core_status: "in_progress",
      filter: null,
    });
    expect(screen.queryByRole("toolbar", { name: "Select" })).not.toBeInTheDocument();
  });

  it("bulk adds a personal tag through the tag dialog", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Steins;Gate" }));
    await userEvent.click(within(actionBar()).getByRole("button", { name: "Add tag" }));

    const input = await screen.findByLabelText("Tag");
    await userEvent.type(input, "Comfort read");
    await userEvent.click(screen.getByRole("button", { name: "Add tag" }));

    expect(invoke).toHaveBeenCalledWith("media_bulk_add_tag", {
      ids: ["m-111"],
      tag: "Comfort read",
      filter: null,
    });
  });

  it("bulk deletes to trash and undoes the whole batch", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Select all" }));
    await userEvent.click(within(actionBar()).getByRole("button", { name: "Delete" }));

    expect(invoke).toHaveBeenCalledWith("media_bulk_delete", {
      ids: ["m-111", "m-222"],
      filter: null,
    });

    const undo = await screen.findByRole("button", { name: "Undo" });
    await userEvent.click(undo);
    expect(invoke).toHaveBeenCalledWith("trash_restore", { id: "t-1" });
    expect(invoke).toHaveBeenCalledWith("trash_restore", { id: "t-2" });
  });

  it("adds selected titles to a collection from the picker", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Steins;Gate" }));
    await userEvent.click(within(actionBar()).getByRole("button", { name: "Add to list" }));
    await userEvent.click(await screen.findByRole("button", { name: "Reading Now" }));

    expect(invoke).toHaveBeenCalledWith("collection_bulk_add", {
      collection_id: "c-1",
      media_ids: ["m-111"],
      filter: null,
    });
  });

  it("shows a hint when no collections exist yet", async () => {
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "media_facets") return Promise.resolve(FACETS);
      if (command === "media_list") return Promise.resolve(TITLES);
      if (command === "collection_list") return Promise.resolve([]);
      return Promise.resolve(null);
    });
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Steins;Gate" }));
    await userEvent.click(within(actionBar()).getByRole("button", { name: "Add to list" }));
    expect(await screen.findByText(/No collections yet/)).toBeInTheDocument();
  });

  it("shows the change summary after a bulk status change", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Select all" }));
    await userEvent.click(within(actionBar()).getByRole("button", { name: "Status" }));
    await userEvent.click(await screen.findByRole("button", { name: "In progress" }));

    expect(await screen.findByText("Status updated — 2 of 2")).toBeInTheDocument();
    expect(screen.queryByText(/skipped/)).not.toBeInTheDocument();
  });

  it("reports skipped titles when some media fail", async () => {
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "media_facets") return Promise.resolve(FACETS);
      if (command === "media_list") return Promise.resolve(TITLES);
      if (command === "tracking_bulk_set_status") {
        return Promise.resolve({
          total: 2,
          succeeded: 1,
          failed: 1,
          failures: [{ media_id: "m-222", reason: "validation error: repeat requires completed" }],
        });
      }
      return Promise.resolve(null);
    });
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Select all" }));
    await userEvent.click(within(actionBar()).getByRole("button", { name: "Status" }));
    await userEvent.click(await screen.findByRole("button", { name: "In progress" }));

    expect(await screen.findByText("Status updated — 1 of 2")).toBeInTheDocument();
    expect(await screen.findByText("1 title was skipped")).toBeInTheDocument();
  });

  it("applies a bulk action to the whole filtered selection", async () => {
    mockLibrary();
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Filter" }));
    await userEvent.click(await screen.findByRole("button", { name: "Fantasy" }));
    await userEvent.keyboard("{Escape}");

    await userEvent.click(await screen.findByRole("button", { name: "Steins;Gate" }));
    await userEvent.click(screen.getByRole("button", { name: "All 2 matching" }));

    await userEvent.click(within(actionBar()).getByRole("button", { name: "Status" }));
    await userEvent.click(await screen.findByRole("button", { name: "In progress" }));

    expect(invoke).toHaveBeenCalledWith("tracking_bulk_set_status", {
      ids: ["m-111"],
      core_status: "in_progress",
      filter: {
        content_type: null,
        format: null,
        pub_status: null,
        genre: "fantasy",
        tag: null,
        year: null,
        favorite: null,
      },
    });
  });

  it("restores only the titles that were actually deleted", async () => {
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "media_facets") return Promise.resolve(FACETS);
      if (command === "media_list") return Promise.resolve(TITLES);
      if (command === "media_bulk_delete") {
        return Promise.resolve({
          summary: {
            total: 2,
            succeeded: 1,
            failed: 1,
            failures: [{ media_id: "m-222", reason: "validation error: media not found" }],
          },
          trash_ids: ["t-1"],
        });
      }
      if (command === "trash_restore") return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
    renderPage();
    await enterSelectMode();

    await userEvent.click(screen.getByRole("button", { name: "Select all" }));
    await userEvent.click(within(actionBar()).getByRole("button", { name: "Delete" }));

    const undo = await screen.findByRole("button", { name: "Undo" });
    await userEvent.click(undo);

    const restoreCalls = vi
      .mocked(invoke)
      .mock.calls.filter(([command]) => command === "trash_restore");
    expect(restoreCalls).toEqual([["trash_restore", { id: "t-1" }]]);
  });
});
