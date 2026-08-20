import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import "@/i18n";
import i18n from "@/i18n";
import type { StatsView } from "@/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { StatsPage } from "./StatsPage";

function stats(overrides: Partial<StatsView> = {}): StatsView {
  return {
    total: 4,
    status_counts: [
      { key: "completed", count: 2 },
      { key: "in_progress", count: 2 },
    ],
    content_type_counts: [
      { key: "anime", count: 2 },
      { key: "book", count: 2 },
    ],
    rating_counts: [
      { key: "9", count: 1 },
      { key: "8", count: 1 },
    ],
    avg_rating: 8.5,
    favorites: 1,
    completed_media: 2,
    completion_rate: 0.5,
    avg_percent: 62,
    consumed_minutes: 150,
    consumed_hours: 2.5,
    consumed_pages: 320,
    year_counts: [{ key: "2011", count: 3 }],
    ...overrides,
  };
}

function wrap(response: unknown) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "stats_summary") return Promise.resolve(response);
    return Promise.resolve([]);
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/stats"]}>
        <Routes>
          <Route path="/stats" element={<StatsPage />} />
          <Route path="*" element={<div>FALLBACK</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function chartSection(title: string) {
  const heading = screen.getByRole("heading", { name: title });
  const section = heading.closest("section");
  if (!section) throw new Error(`no section for ${title}`);
  return section;
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("StatsPage", () => {
  it("renders stat cards with tabular values", async () => {
    wrap(stats());
    expect(await screen.findByText("Titles tracked")).toBeInTheDocument();
    expect(screen.getByText("4")).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();
    expect(screen.getByText("8.5")).toBeInTheDocument();
    expect(screen.getByText("2.5")).toBeInTheDocument();
    expect(screen.getByText("320")).toBeInTheDocument();
  });

  it("renders the four distribution charts with counts", async () => {
    wrap(stats());
    expect(await screen.findByRole("heading", { name: "By status" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "By content type" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "By rating" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "By release year" })).toBeInTheDocument();

    const status = chartSection("By status");
    expect(within(status).getByText("Completed")).toBeInTheDocument();
    expect(within(status).getByText("In progress")).toBeInTheDocument();

    const types = chartSection("By content type");
    expect(within(types).getByText("Anime")).toBeInTheDocument();
    expect(within(types).getByText("Book")).toBeInTheDocument();
  });

  it("shows a calm empty state when nothing is tracked", async () => {
    wrap(stats({ total: 0 }));
    expect(await screen.findByText("No stats yet")).toBeInTheDocument();
  });

  it("surfaces an error with retry when the summary fails", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("boom"));
    wrap(stats());
    expect(await screen.findByText("Couldn't load your stats")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Titles tracked")).toBeInTheDocument();
  });
});
