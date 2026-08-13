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
import { TrackingTab } from "./TrackingTab";
import type { TrackingView } from "@/api";

const VIEW: TrackingView = {
  media_id: "m-111",
  core_status: "completed",
  custom_status_id: null,
  started_at: "2026-01-01T00:00:00Z",
  finished_at: "2026-01-03T00:00:00Z",
  repeat_count: 0,
  updated_at: "2026-01-03T00:00:00Z",
};

function renderTab() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <TrackingTab mediaId="m-111" />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

function mockTracking(view: TrackingView | null) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "tracking_get") return Promise.resolve(view);
    return Promise.resolve(undefined);
  });
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("TrackingTab", () => {
  it("renders all status pills with the current one selected", async () => {
    mockTracking(VIEW);
    renderTab();

    const completed = await screen.findByRole("button", { name: "Completed" });
    expect(completed).toHaveAttribute("aria-pressed", "true");
    for (const name of ["Planned", "In progress", "On hold", "Dropped", "Repeat", "Wishlist"]) {
      expect(screen.getByRole("button", { name })).toHaveAttribute("aria-pressed", "false");
    }
    expect(screen.queryByText("Re-read run")).not.toBeInTheDocument();
  });

  it("shows the untracked hint when there is no tracking row", async () => {
    mockTracking(null);
    renderTab();
    expect(
      await screen.findByText("Not tracked yet — pick a status above to start."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Planned" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("applies a status transition on click and reflects the returned row", async () => {
    mockTracking(null);
    renderTab();
    await screen.findByText("Not tracked yet — pick a status above to start.");

    vi.mocked(invoke).mockResolvedValueOnce({
      ...VIEW,
      core_status: "on_hold",
      started_at: "2026-02-01T00:00:00Z",
      finished_at: null,
    });
    await userEvent.click(screen.getByRole("button", { name: "On hold" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("tracking_set_status", {
        media_id: "m-111",
        core_status: "on_hold",
      }),
    );
    expect(await screen.findByRole("button", { name: "On hold" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("shows the repeat run counter only while re-reading", async () => {
    mockTracking({ ...VIEW, core_status: "repeat", repeat_count: 2, finished_at: null });
    renderTab();
    expect(await screen.findByText("Re-read run")).toBeInTheDocument();
    expect(screen.getByText("#2")).toBeInTheDocument();
    expect(screen.queryByText("Finished")).not.toBeInTheDocument();
  });

  it("renders the started and finished dates", async () => {
    mockTracking(VIEW);
    renderTab();
    expect(await screen.findByText("Started")).toBeInTheDocument();
    expect(screen.getByText("Finished")).toBeInTheDocument();
    expect(screen.getByText("January 1, 2026")).toBeInTheDocument();
    expect(screen.getByText("January 3, 2026")).toBeInTheDocument();
  });

  it("shows an error state with retry when loading fails", async () => {
    vi.mocked(invoke)
      .mockRejectedValueOnce("boom")
      .mockImplementation((cmd: string) => {
        if (cmd === "tracking_get") return Promise.resolve(null);
        return Promise.resolve(undefined);
      });
    renderTab();

    const retry = await screen.findByRole("button", { name: "Retry" });
    await userEvent.click(retry);
    expect(
      await screen.findByText("Not tracked yet — pick a status above to start."),
    ).toBeInTheDocument();
  });

  it("shows an error toast when the transition is rejected", async () => {
    mockTracking(null);
    renderTab();
    await screen.findByText("Not tracked yet — pick a status above to start.");

    vi.mocked(invoke).mockRejectedValueOnce("boom");
    await userEvent.click(screen.getByRole("button", { name: "Repeat" }));

    expect(await screen.findByText("Couldn't update the status")).toBeInTheDocument();
  });
});
