import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { StatusBar } from "./StatusBar";

/* MISSION-146 — the status-bar title count is live, not hardcoded. */

function renderBar() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <StatusBar />
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("StatusBar", () => {
  it("renders the live title count from media_count", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "media_count") return 42;
      throw new Error(`unexpected command ${cmd}`);
    });
    renderBar();

    expect(await screen.findByText("42 titles")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("media_count");
  });

  it("uses the singular form for one title", async () => {
    vi.mocked(invoke).mockResolvedValue(1);
    renderBar();

    expect(await screen.findByText("1 title")).toBeInTheDocument();
  });

  it("falls back to zero before the count loads", async () => {
    vi.mocked(invoke).mockResolvedValue(0);
    renderBar();

    expect(await screen.findByText("0 titles")).toBeInTheDocument();
  });
});
