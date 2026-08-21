import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import { MediaRow } from "./MediaRow";
import { NO_PROGRESS } from "./testFixtures";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

/* MISSION-092 — Mixed-direction titles: media titles are user/provider data
   and may be Latin inside an Arabic UI (or vice versa), so the row title
   carries dir="auto" to pick its own direction. */

const ITEM = {
  id: "m-1",
  content_type: "anime",
  title: "Steins;Gate",
  pub_status: "completed",
  release_year: 2011,
  cover_asset_id: null,
  updated_at: "2026-01-01T00:00:00Z",
  favorite: false,
  progress: NO_PROGRESS,
};

function renderRow() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <ToastProvider>
          <MediaRow item={ITEM} />
        </ToastProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("MediaRow", () => {
  it("renders the title with automatic direction", () => {
    renderRow();
    const title = screen.getByRole("heading", { name: "Steins;Gate" });
    expect(title).toHaveAttribute("dir", "auto");
  });
});
