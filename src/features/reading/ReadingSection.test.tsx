import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@/i18n";
import i18n from "@/i18n";
import type { ReadingRecap } from "@/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { ReadingSection } from "./ReadingSection";

const CURRENT_YEAR = new Date().getFullYear();

function emptyRecap(year: number): ReadingRecap {
  return {
    year,
    by_month: Array.from({ length: 12 }, () => ({ pages: 0, chapters: 0 })),
    totals: { pages: 0, chapters: 0, finished: 0 },
    mood_counts: [],
    pace_counts: [],
    format_counts: [],
  };
}

function fullRecap(year: number): ReadingRecap {
  const by_month = Array.from({ length: 12 }, () => ({ pages: 0, chapters: 0 }));
  by_month[2] = { pages: 240, chapters: 2 };
  by_month[5] = { pages: 80, chapters: 3 };
  return {
    year,
    by_month,
    totals: { pages: 320, chapters: 5, finished: 1 },
    mood_counts: [
      { key: "dark", count: 2 },
      { key: "tense", count: 1 },
    ],
    pace_counts: [{ key: "medium", count: 2 }],
    format_counts: [{ key: "light_novel", count: 3 }],
  };
}

function wrap(makeRecap: (year: number) => ReadingRecap = fullRecap) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: unknown) => {
    if (cmd === "reading_recap") {
      const payload = args as Record<string, unknown> | undefined;
      const year = typeof payload?.year === "number" ? payload.year : CURRENT_YEAR;
      return Promise.resolve(makeRecap(year));
    }
    return Promise.resolve([]);
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ReadingSection />
    </QueryClientProvider>,
  );
}

function card(label: string): HTMLElement {
  const labelNode = screen.getByText(label);
  const cardNode = labelNode.closest("div");
  if (!cardNode) throw new Error(`no card for ${label}`);
  return cardNode;
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("ReadingSection", () => {
  it("renders year totals, month charts and taste distributions", async () => {
    wrap();

    expect(await screen.findByRole("heading", { name: "Reading recap" })).toBeInTheDocument();
    await screen.findByText("320");
    expect(within(card("Pages read")).getByText("320")).toBeInTheDocument();
    expect(within(card("Chapters read")).getByText("5")).toBeInTheDocument();
    expect(within(card("Finished")).getByText("1")).toBeInTheDocument();

    expect(screen.getByRole("heading", { name: "Pages per month" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Chapters per month" })).toBeInTheDocument();

    const moods = screen.getByRole("heading", { name: "Moods" }).closest("section");
    expect(within(moods!).getByText("Dark")).toBeInTheDocument();
    expect(within(moods!).getByText("Tense")).toBeInTheDocument();

    const pace = screen.getByRole("heading", { name: "Pace" }).closest("section");
    expect(within(pace!).getByText("Medium")).toBeInTheDocument();

    const formats = screen.getByRole("heading", { name: "Formats" }).closest("section");
    expect(within(formats!).getByText("light_novel")).toBeInTheDocument();
  });

  it("switches the year and reloads the recap", async () => {
    const user = userEvent.setup();
    wrap((year) =>
      year === 2025
        ? { ...fullRecap(year), totals: { pages: 100, chapters: 1, finished: 0 } }
        : fullRecap(year),
    );

    const select = await screen.findByRole("combobox");
    await user.selectOptions(select, "2025");

    expect(await within(card("Pages read")).findByText("100")).toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("reading_recap", { year: 2025 });
  });

  it("shows a calm note when the year has no reading", async () => {
    wrap(emptyRecap);
    expect(await screen.findByText("No reading activity in this year.")).toBeInTheDocument();
  });

  it("surfaces an error with a retry that recovers", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("boom"));
    wrap();

    expect(await screen.findByText("Couldn't load the reading recap")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("heading", { name: "Reading recap" })).toBeInTheDocument();
    expect(within(card("Pages read")).getByText("320")).toBeInTheDocument();
  });
});
