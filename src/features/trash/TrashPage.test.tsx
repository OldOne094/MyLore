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
import { TrashPage } from "./TrashPage";

const ITEMS = [
  {
    id: "t-1",
    kind: "media",
    title: "Steins;Gate",
    deleted_at: "2026-01-01T00:00:00Z",
  },
  {
    id: "t-2",
    kind: "media",
    title: "Sword of the Dawn",
    deleted_at: "2026-01-02T00:00:00Z",
  },
];

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={["/trash"]}>
          <Routes>
            <Route path="/trash" element={<TrashPage />} />
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

describe("TrashPage", () => {
  it("shows an empty state when the trash is empty", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    renderPage();
    expect(await screen.findByText("Trash is empty")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("trash_list");
  });

  it("lists trashed titles with restore and delete-forever actions", async () => {
    vi.mocked(invoke).mockResolvedValue(ITEMS);
    renderPage();

    expect(await screen.findByText("Steins;Gate")).toBeInTheDocument();
    expect(screen.getByText("Sword of the Dawn")).toBeInTheDocument();
    expect(screen.getByText("2 trashed titles")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Restore" })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "Delete forever" })).toHaveLength(2);
  });

  it("restores an item and confirms with a toast", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "trash_list") return Promise.resolve([ITEMS[0]]);
      return Promise.resolve(undefined);
    });
    renderPage();

    const restore = await screen.findByRole("button", { name: "Restore" });
    await userEvent.click(restore);

    expect(invoke).toHaveBeenCalledWith("trash_restore", { id: "t-1" });
    expect(await screen.findByText("Restored “Steins;Gate”")).toBeInTheDocument();
  });

  it("confirms a permanent purge before deleting forever", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "trash_list") return Promise.resolve([ITEMS[0]]);
      return Promise.resolve(undefined);
    });
    renderPage();

    await userEvent.click(await screen.findByRole("button", { name: "Delete forever" }));

    expect(await screen.findByRole("heading", { name: "Delete forever?" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Confirm deleting forever" }));

    expect(invoke).toHaveBeenCalledWith("trash_purge", { id: "t-1" });
    expect(await screen.findByText("Deleted “Steins;Gate” forever")).toBeInTheDocument();
  });

  it("shows a retry state when loading fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("boom").mockResolvedValueOnce(ITEMS);
    renderPage();

    const retry = await screen.findByRole("button", { name: "Retry" });
    await userEvent.click(retry);

    expect(await screen.findByText("Steins;Gate")).toBeInTheDocument();
  });
});
