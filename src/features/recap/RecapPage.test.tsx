import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import "@/i18n";
import i18n from "@/i18n";
import type { YearRecap } from "@/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { RecapPage } from "./RecapPage";

const NOW = new Date().getFullYear();

function recap(year: number, overrides: Partial<YearRecap> = {}): YearRecap {
  return {
    year,
    totals: { added: 12, started: 10, completed: 8, reviewed: 5, progress: 40 },
    by_month: [0, 0, 0, 0, 0, 3, 2, 0, 1, 2, 0, 0],
    top_genres: [{ name: "Fantasy", count: 3 }],
    top_media: [
      { media_id: "m-1", title: "Series", content_type: "anime", activity_count: 20 },
      { media_id: "m-2", title: "Book", content_type: "novel", activity_count: 12 },
    ],
    longest_streak: 7,
    best_month: 6,
    ...overrides,
  };
}

function wrap(response?: YearRecap) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: InvokeArgs) => {
    if (cmd === "recap_year") {
      const a = args as Record<string, unknown> | undefined;
      const year = (a?.year as number) ?? NOW;
      return Promise.resolve(response ?? recap(year));
    }
    return Promise.resolve([]);
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/recap"]}>
        <Routes>
          <Route path="/recap" element={<RecapPage />} />
          <Route path="/library/:id" element={<div>MEDIA_PAGE</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("RecapPage", () => {
  it("renders headline totals and highlights", async () => {
    wrap(recap(NOW));
    expect(await screen.findByText(`Your ${NOW} in review`)).toBeInTheDocument();
    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("10")).toBeInTheDocument();
    expect(screen.getByText("8")).toBeInTheDocument();
    expect(screen.getByText("40")).toBeInTheDocument();
    expect(screen.getByText("June")).toBeInTheDocument();
    expect(screen.getByText("7")).toBeInTheDocument();
  });

  it("renders the monthly chart with best month highlighted", async () => {
    wrap(recap(NOW));
    const chart = await screen.findByRole("region", { name: "Finishes by month" });
    expect(within(chart).getByText("Jan")).toBeInTheDocument();
    expect(within(chart).getByText("Dec")).toBeInTheDocument();
    expect(within(chart).getByText("3")).toBeInTheDocument();
  });

  it("lists genres and most-active titles with links", async () => {
    wrap(recap(NOW));
    expect(await screen.findByRole("heading", { name: "Top genres" })).toBeInTheDocument();
    expect(screen.getByText("Fantasy")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Most active titles" })).toBeInTheDocument();
    const series = screen.getByRole("link", { name: "Series" });
    expect(series).toHaveAttribute("href", "/library/m-1");
    expect(screen.getByText("×20")).toBeInTheDocument();
  });

  it("switches years via the selector and refetches", async () => {
    const user = userEvent.setup();
    wrap();
    expect(await screen.findByText(`Your ${NOW} in review`)).toBeInTheDocument();
    const select = screen.getByRole("combobox", { name: "Year" });
    await user.selectOptions(select, String(NOW - 1));
    expect(await screen.findByText(`Your ${NOW - 1} in review`)).toBeInTheDocument();
  });

  it("shows a calm empty state for a quiet year", async () => {
    wrap(
      recap(NOW, {
        totals: { added: 0, started: 0, completed: 0, reviewed: 0, progress: 0 },
        by_month: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        top_genres: [],
        top_media: [],
        longest_streak: 0,
        best_month: null,
      }),
    );
    expect(await screen.findByText("No activity this year")).toBeInTheDocument();
  });

  it("surfaces an error with retry when the recap fails", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("boom"));
    wrap();
    expect(await screen.findByText("Couldn't load the recap")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText(`Your ${NOW} in review`)).toBeInTheDocument();
  });
});
