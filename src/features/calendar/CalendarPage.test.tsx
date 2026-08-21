import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import "@/i18n";
import i18n from "@/i18n";
import type { CalendarDay, CalendarItem, CalendarMonth } from "@/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { CalendarPage } from "./CalendarPage";

function monthData(
  year: number,
  month: number,
  overrides: Record<number, Partial<CalendarDay>> = {},
): CalendarMonth {
  const count = new Date(year, month, 0).getDate();
  const days: CalendarDay[] = Array.from({ length: count }, (_, index) => {
    const day = index + 1;
    return {
      date: `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`,
      airs: [],
      activity: [],
      ...overrides[day],
    };
  });
  return { year, month, days };
}

function airItem(): CalendarItem {
  return {
    media_id: "m-1",
    title: "Series",
    content_type: "anime",
    label: "E5",
    kind: null,
    time: null,
  };
}

function activityItem(): CalendarItem {
  return {
    media_id: "m-2",
    title: "Book",
    content_type: "novel",
    label: null,
    kind: "started",
    time: "09:00",
  };
}

const calls: { year: number; month: number }[] = [];

function wrap(response?: CalendarMonth | null) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: InvokeArgs) => {
    if (cmd === "calendar_month") {
      const a = args as Record<string, unknown> | undefined;
      const year = (a?.year as number) ?? 0;
      const month = (a?.month as number) ?? 0;
      calls.push({ year, month });
      return Promise.resolve(response ?? monthData(year, month));
    }
    return Promise.resolve([]);
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/calendar"]}>
        <Routes>
          <Route path="/calendar" element={<CalendarPage />} />
          <Route path="/library/:id" element={<div>MEDIA_PAGE</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  calls.length = 0;
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("CalendarPage", () => {
  it("renders a loading skeleton while the month is in flight (MISSION-091)", async () => {
    let resolveMonth: ((value: CalendarMonth) => void) | undefined;
    vi.mocked(invoke).mockImplementation(
      () =>
        new Promise<CalendarMonth>((resolve) => {
          resolveMonth = resolve;
        }),
    );
    render(
      <QueryClientProvider
        client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
      >
        <MemoryRouter initialEntries={["/calendar"]}>
          <Routes>
            <Route path="/calendar" element={<CalendarPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(screen.getByRole("status")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "1" })).not.toBeInTheDocument();

    resolveMonth?.(monthData(2026, 8));
    expect(await screen.findByRole("button", { name: "1" })).toBeInTheDocument();
  });

  it("renders the month grid with localized weekday headers", async () => {
    wrap();
    expect(await screen.findByRole("button", { name: "1" })).toBeInTheDocument();
    expect(screen.getByText("Sun")).toBeInTheDocument();
    expect(screen.getByText("Sat")).toBeInTheDocument();
  });

  it("lists air and activity events for the selected day", async () => {
    const user = userEvent.setup();
    const now = new Date();
    wrap(
      monthData(now.getFullYear(), now.getMonth() + 1, {
        5: { airs: [airItem()], activity: [activityItem()] },
      }),
    );
    await user.click(await screen.findByRole("button", { name: "5" }));
    expect(await screen.findByText("Series")).toBeInTheDocument();
    expect(screen.getByText("E5")).toBeInTheDocument();
    expect(screen.getByText("Started")).toBeInTheDocument();
    expect(screen.getByText("09:00")).toBeInTheDocument();
    expect(screen.getByText("Book")).toBeInTheDocument();
  });

  it("shows a calm empty message for a day with no events", async () => {
    const user = userEvent.setup();
    const now = new Date();
    wrap(
      monthData(now.getFullYear(), now.getMonth() + 1, {
        5: { airs: [airItem()], activity: [activityItem()] },
      }),
    );
    await user.click(await screen.findByRole("button", { name: "6" }));
    expect(await screen.findByText("Nothing on this day.")).toBeInTheDocument();
  });

  it("navigates months and refetches", async () => {
    const user = userEvent.setup();
    wrap();
    await screen.findByRole("button", { name: "1" });
    await user.click(screen.getByRole("button", { name: "Next month" }));
    const now = new Date();
    const expected = new Date(now.getFullYear(), now.getMonth() + 1, 1);
    expect(calls[calls.length - 1]).toEqual({
      year: expected.getFullYear(),
      month: expected.getMonth() + 1,
    });
  });

  it("surfaces an error with retry when the month fails", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("boom"));
    wrap();
    expect(await screen.findByText("Couldn't load the calendar")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("button", { name: "1" })).toBeInTheDocument();
  });
});
