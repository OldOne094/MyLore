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
import { ReviewsPage } from "./ReviewsPage";

const ITEMS = [
  {
    media_id: "m-1",
    title: "Steins;Gate",
    content_type: "anime",
    cover_asset_id: null,
    rating: 9,
    review: "Time travel done right — a slow burn that pays off.",
    short_review: null,
    notes: null,
    favorite: false,
    is_spoiler: false,
    moods: ["tense", "dark"],
    pace: "medium",
    content_warnings: [],
    warnings_acknowledged_at: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-02-03T00:00:00Z",
  },
  {
    media_id: "m-2",
    title: "Sword of the Dawn",
    content_type: "novel",
    cover_asset_id: null,
    rating: 7,
    review: "The twist at the end changes everything.",
    short_review: null,
    notes: null,
    favorite: true,
    is_spoiler: true,
    moods: [],
    pace: null,
    content_warnings: [],
    warnings_acknowledged_at: null,
    created_at: "2026-01-05T00:00:00Z",
    updated_at: "2026-01-05T00:00:00Z",
  },
];

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={["/reviews"]}>
          <Routes>
            <Route path="/reviews" element={<ReviewsPage />} />
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

describe("ReviewsPage", () => {
  it("shows an empty state when nothing has been reviewed", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    renderPage();
    expect(await screen.findByText("No reviews yet")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("review_list");
  });

  it("lists reviews with title, rating, review text and metadata", async () => {
    vi.mocked(invoke).mockResolvedValue(ITEMS);
    renderPage();

    expect(await screen.findByText("Steins;Gate")).toBeInTheDocument();
    expect(screen.getByText("Sword of the Dawn")).toBeInTheDocument();
    expect(screen.getByText("9/10")).toBeInTheDocument();
    expect(
      screen.getByText("Time travel done right — a slow burn that pays off."),
    ).toBeInTheDocument();
    expect(screen.getByText("Tense")).toBeInTheDocument();
    expect(screen.getByText("Medium")).toBeInTheDocument();
  });

  it("flags spoiler reviews and links rows into the library", async () => {
    vi.mocked(invoke).mockResolvedValue(ITEMS);
    renderPage();

    expect(await screen.findByText("Spoilers")).toBeInTheDocument();
    const links = screen.getAllByRole("link");
    expect(links.some((link) => link.getAttribute("href") === "/library/m-1")).toBe(true);
    expect(links.some((link) => link.getAttribute("href") === "/library/m-2")).toBe(true);
  });

  it("shows a retry state when loading fails and recovers", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("boom").mockResolvedValueOnce(ITEMS);
    renderPage();

    const retry = await screen.findByRole("button", { name: "Retry" });
    await userEvent.click(retry);

    expect(await screen.findByText("Steins;Gate")).toBeInTheDocument();
  });
});
