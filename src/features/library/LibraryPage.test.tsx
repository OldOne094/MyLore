import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { LibraryPage } from "./LibraryPage";

const TITLES = [
  {
    id: "m-111",
    content_type: "anime",
    title: "Steins;Gate",
    pub_status: "completed",
    release_year: 2011,
    cover_asset_id: null,
    updated_at: "2026-01-01T00:00:00Z",
  },
  {
    id: "m-222",
    content_type: "novel",
    title: "Sword of the Dawn",
    pub_status: "ongoing",
    release_year: 2026,
    cover_asset_id: null,
    updated_at: "2026-01-02T00:00:00Z",
  },
];

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <LibraryPage />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("LibraryPage", () => {
  it("renders the empty state when there are no titles", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    renderPage();
    expect(await screen.findByText("Your library is empty")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add title" })).toBeInTheDocument();
  });

  it("renders a grid of cards for existing titles", async () => {
    vi.mocked(invoke).mockResolvedValue(TITLES);
    renderPage();

    expect(await screen.findByText("Steins;Gate")).toBeInTheDocument();
    expect(screen.getByText("Sword of the Dawn")).toBeInTheDocument();
    expect(screen.getAllByRole("article")).toHaveLength(2);
    expect(screen.getByText("Anime")).toBeInTheDocument();
    expect(screen.getByText("Novel")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("Ongoing")).toBeInTheDocument();
    expect(screen.queryByText("Your library is empty")).not.toBeInTheDocument();
  });

  it("requests the library listing with default args", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    renderPage();
    await screen.findByText("Your library is empty");

    expect(invoke).toHaveBeenCalledWith(
      "media_list",
      expect.objectContaining({ sort: "title", ascending: true, content_type: null }),
    );
  });

  it("shows a retry action when loading fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("boom");
    vi.mocked(invoke).mockResolvedValueOnce(TITLES);
    renderPage();

    const retry = await screen.findByRole("button", { name: "Retry" });
    await userEvent.click(retry);

    await waitFor(() => expect(screen.getByText("Steins;Gate")).toBeInTheDocument());
  });
});
