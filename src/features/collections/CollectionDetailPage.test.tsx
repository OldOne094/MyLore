import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { CollectionDetailPage } from "./CollectionDetailPage";
import { NO_PROGRESS } from "@/features/library/testFixtures";
import type { MediaListItem } from "@/features/library/api";

/* MISSION-076/077 — Collection detail. Ordered members over `collection_members`:
   reorder via native HTML5 drag-and-drop (jsdom fireEvent) or the Up/Down
   buttons, remove via `collection_remove_member`, all wired to
   `collection_reorder` with the full ordered id list. MISSION-077 adds the
   smart branch: computed members render read-only with an "Edit filter" dialog
   that writes `collection_update_smart`. */

const TITLE_A: MediaListItem = {
  id: "m-1",
  content_type: "novel",
  title: "Dune",
  pub_status: "completed",
  release_year: 1965,
  cover_asset_id: null,
  updated_at: "2026-01-01T00:00:00Z",
  favorite: false,
  progress: NO_PROGRESS,
};

const TITLE_B: MediaListItem = {
  id: "m-2",
  content_type: "anime",
  title: "Berserk",
  pub_status: "ongoing",
  release_year: 1997,
  cover_asset_id: null,
  updated_at: "2026-01-02T00:00:00Z",
  favorite: true,
  progress: NO_PROGRESS,
};

const MEMBERS = [
  { position: 0, media: TITLE_A },
  { position: 1, media: TITLE_B },
];

const COLLECTION = {
  id: "c-1",
  name: "Reading Now",
  member_count: 2,
  created_at: "2026-01-01",
};

function mockDetail(members: typeof MEMBERS = MEMBERS) {
  vi.mocked(invoke).mockImplementation((command: string, args?: InvokeArgs) => {
    const a = (args ?? {}) as { collection_id?: string; media_id?: string; media_ids?: string[] };
    if (command === "collection_list") return Promise.resolve([COLLECTION]);
    if (command === "collection_members") return Promise.resolve(members);
    if (command === "collection_remove_member") {
      const mediaId = a.media_id as string;
      members = members.filter((m) => m.media.id !== mediaId);
      return Promise.resolve(mediaId);
    }
    if (command === "collection_reorder") {
      const ids = a.media_ids as string[];
      members = [...members].sort((a, b) => ids.indexOf(a.media.id) - ids.indexOf(b.media.id));
      return Promise.resolve(undefined);
    }
    return Promise.resolve(null);
  });
}

const SMART_COLLECTION = {
  id: "c-smart",
  name: "Anime shelf",
  is_smart: true,
  filter: {
    content_type: "anime",
    format: null,
    pub_status: null,
    genre: null,
    tag: null,
    year: null,
    favorite: null,
    sort: "title",
    ascending: true,
  },
  member_count: 2,
  created_at: "2026-01-01",
};

function mockSmartDetail(members: typeof MEMBERS = MEMBERS) {
  vi.mocked(invoke).mockImplementation((command: string, args?: InvokeArgs) => {
    const a = (args ?? {}) as { filter?: unknown };
    if (command === "collection_list") return Promise.resolve([SMART_COLLECTION]);
    if (command === "collection_members") return Promise.resolve(members);
    if (command === "collection_update_smart") {
      return Promise.resolve({ ...SMART_COLLECTION, filter: a.filter });
    }
    return Promise.resolve(null);
  });
}

function renderPage(collectionId = "c-1") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={[`/collections/${collectionId}`]}>
          <Routes>
            <Route path="/collections/:collectionId" element={<CollectionDetailPage />} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

/** Member links in display order (excludes the "Back to collections" link). */
function memberTitles() {
  return screen
    .getAllByRole("link")
    .filter((link) => link.getAttribute("aria-label") !== "Back to collections")
    .map((link) => link.getAttribute("aria-label"));
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("CollectionDetailPage", () => {
  it("renders the collection with its members in order", async () => {
    mockDetail();
    renderPage();

    expect(await screen.findByRole("heading", { name: "Reading Now" })).toBeInTheDocument();
    expect(screen.getByText("2 titles")).toBeInTheDocument();
    expect(memberTitles()).toEqual(["Dune", "Berserk"]);
    expect(screen.getByRole("link", { name: "Dune" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to collections" })).toBeInTheDocument();
  });

  it("reorders via the move-down button", async () => {
    mockDetail();
    renderPage();
    await screen.findByRole("heading", { name: "Reading Now" });

    await userEvent.click(screen.getAllByRole("button", { name: "Move down" })[0]);
    await screen.findByText("Order saved");

    expect(invoke).toHaveBeenCalledWith("collection_reorder", {
      collection_id: "c-1",
      media_ids: ["m-2", "m-1"],
    });
    expect(memberTitles()).toEqual(["Berserk", "Dune"]);
  });

  it("reorders via drag-and-drop", async () => {
    mockDetail();
    renderPage();
    await screen.findByRole("heading", { name: "Reading Now" });

    const duneRow = screen.getByRole("link", { name: "Dune" }).closest("[draggable]");
    const berserkRow = screen.getByRole("link", { name: "Berserk" }).closest("[draggable]");
    expect(duneRow).not.toBeNull();
    expect(berserkRow).not.toBeNull();

    fireEvent.dragStart(duneRow as HTMLElement);
    fireEvent.dragOver(berserkRow as HTMLElement);
    fireEvent.drop(berserkRow as HTMLElement);
    fireEvent.dragEnd(duneRow as HTMLElement);
    await screen.findByText("Order saved");

    expect(invoke).toHaveBeenCalledWith("collection_reorder", {
      collection_id: "c-1",
      media_ids: ["m-2", "m-1"],
    });
    expect(memberTitles()).toEqual(["Berserk", "Dune"]);
  });

  it("disables move buttons at the edges", async () => {
    mockDetail();
    renderPage();
    await screen.findByRole("heading", { name: "Reading Now" });

    const moveUpButtons = screen.getAllByRole("button", { name: "Move up" });
    const moveDownButtons = screen.getAllByRole("button", { name: "Move down" });
    expect(moveUpButtons[0]).toBeDisabled();
    expect(moveDownButtons[moveDownButtons.length - 1]).toBeDisabled();
  });

  it("removes a member", async () => {
    mockDetail();
    renderPage();
    await screen.findByRole("heading", { name: "Reading Now" });

    await userEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);

    expect(invoke).toHaveBeenCalledWith("collection_remove_member", {
      collection_id: "c-1",
      media_id: "m-1",
    });
    await screen.findByText("Removed “Dune”");
    expect(memberTitles()).toEqual(["Berserk"]);
    expect(screen.getByText("Removed “Dune”")).toBeInTheDocument();
  });

  it("shows the empty detail state when the collection has no members", async () => {
    mockDetail([]);
    renderPage();

    expect(await screen.findByText("This collection is empty")).toBeInTheDocument();
  });

  it("shows a not-found state for an unknown collection", async () => {
    mockDetail();
    renderPage("c-nope");

    expect(await screen.findByText("Collection not found")).toBeInTheDocument();
  });

  it("renders a smart collection read-only with the computed note", async () => {
    mockSmartDetail();
    renderPage("c-smart");

    expect(await screen.findByRole("heading", { name: "Anime shelf" })).toBeInTheDocument();
    expect(screen.getByText("Computed from filters")).toBeInTheDocument();
    expect(screen.getAllByLabelText("Smart collection")).toHaveLength(1);
    expect(memberTitles()).toEqual(["Dune", "Berserk"]);
    expect(screen.queryByRole("button", { name: "Move up" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Move down" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
  });

  it("edits a smart collection's filter", async () => {
    mockSmartDetail();
    renderPage("c-smart");
    await screen.findByRole("heading", { name: "Anime shelf" });

    await userEvent.click(screen.getByRole("button", { name: "Edit filter" }));
    const dialog = screen.getByRole("dialog");
    await userEvent.selectOptions(within(dialog).getByLabelText("Status"), "completed");
    await userEvent.click(within(dialog).getByRole("button", { name: "Save collection" }));

    expect(invoke).toHaveBeenCalledWith(
      "collection_update_smart",
      expect.objectContaining({
        collection_id: "c-smart",
        filter: expect.objectContaining({ pub_status: "completed" }),
      }),
    );
    expect(await screen.findByText("Filter updated")).toBeInTheDocument();
  });

  it("shows the smart empty state when nothing matches", async () => {
    mockSmartDetail([]);
    renderPage("c-smart");

    expect(await screen.findByText("Nothing matches these filters")).toBeInTheDocument();
    expect(screen.getByText("Adjust the filter to change what appears here.")).toBeInTheDocument();
  });
});
