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

import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { CollectionsPage } from "./CollectionsPage";

/* MISSION-076 — Collections page. Card grid over `collection_list` with
   create/rename/delete dialogs wired to their IPC commands. */

const VIEWS = [
  { id: "c-1", name: "Reading Now", member_count: 2, created_at: "2026-01-01" },
  { id: "c-2", name: "Watchlist", member_count: 0, created_at: "2026-01-02" },
];

function mockCollections(initial: typeof VIEWS = VIEWS) {
  let list = [...initial];
  vi.mocked(invoke).mockImplementation((command: string, args?: InvokeArgs) => {
    const a = (args ?? {}) as { name?: string; collection_id?: string };
    if (command === "collection_list") return Promise.resolve(list);
    if (command === "collection_create") {
      const name = a.name ?? "";
      const view = { id: "c-new", name, member_count: 0, created_at: "2026-01-03" };
      list = [...list, view];
      return Promise.resolve(view);
    }
    if (command === "collection_rename") {
      const id = a.collection_id as string;
      const name = a.name as string;
      list = list.map((c) => (c.id === id ? { ...c, name } : c));
      return Promise.resolve(list.find((c) => c.id === id));
    }
    if (command === "collection_delete") {
      const id = a.collection_id as string;
      list = list.filter((c) => c.id !== id);
      return Promise.resolve("Reading Now");
    }
    return Promise.resolve(null);
  });
  return { reset: () => (list = [...initial]) };
}

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter>
          <CollectionsPage />
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("CollectionsPage", () => {
  it("renders the collections grid with member counts", async () => {
    mockCollections();
    renderPage();

    expect(await screen.findAllByRole("link", { name: "Open collection" })).toHaveLength(2);
    expect(screen.getByRole("heading", { name: "Reading Now" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Watchlist" })).toBeInTheDocument();
    expect(screen.getAllByText("2 titles")).toHaveLength(2);
    expect(screen.getByText("0 titles")).toBeInTheDocument();
  });

  it("shows the empty state when there are no collections", async () => {
    mockCollections([]);
    renderPage();

    expect(await screen.findByText("No collections yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New collection" })).toBeInTheDocument();
  });

  it("creates a collection from the dialog", async () => {
    mockCollections();
    renderPage();
    await screen.findAllByRole("link", { name: "Open collection" });

    await userEvent.click(screen.getByRole("button", { name: "New collection" }));
    const dialog = screen.getByRole("dialog");
    await userEvent.type(within(dialog).getByLabelText("Name"), "TBR pile");
    await userEvent.click(within(dialog).getByRole("button", { name: "Create" }));

    expect(invoke).toHaveBeenCalledWith("collection_create", { name: "TBR pile" });
    expect(await screen.findByRole("heading", { name: "TBR pile" })).toBeInTheDocument();
    expect(screen.getByText("Created “TBR pile”")).toBeInTheDocument();
  });

  it("renames a collection from its card", async () => {
    mockCollections();
    renderPage();
    await screen.findByRole("heading", { name: "Reading Now" });

    await userEvent.click(screen.getAllByRole("button", { name: "Rename" })[0]);
    const dialog = screen.getByRole("dialog");
    const field = within(dialog).getByLabelText("Name");
    await userEvent.clear(field);
    await userEvent.type(field, "Currently reading");
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    expect(invoke).toHaveBeenCalledWith("collection_rename", {
      collection_id: "c-1",
      name: "Currently reading",
    });
    expect(await screen.findByRole("heading", { name: "Currently reading" })).toBeInTheDocument();
  });

  it("deletes a collection after confirming", async () => {
    mockCollections();
    renderPage();
    await screen.findByRole("heading", { name: "Reading Now" });

    await userEvent.click(screen.getAllByRole("button", { name: "Delete" })[0]);
    const dialog = screen.getByRole("dialog");
    await userEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

    expect(invoke).toHaveBeenCalledWith("collection_delete", { collection_id: "c-1" });
    expect(await screen.findByText("Deleted “Reading Now”")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Reading Now" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Watchlist" })).toBeInTheDocument();
  });

  it("shows an error state with retry", async () => {
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "collection_list") return Promise.reject("boom");
      return Promise.resolve(null);
    });
    renderPage();

    expect(await screen.findByText("Couldn't load your collections")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
