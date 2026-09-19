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

/** The bar hosts the task centre (MISSION-155), which reads `task_list`; answer
    it explicitly so a count-shaped reply never leaks into the list. */
function countOnly(count: number) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "task_list") return [];
    if (cmd === "media_count") return count;
    throw new Error(`unexpected command ${cmd}`);
  });
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("StatusBar", () => {
  it("renders the live title count from media_count", async () => {
    countOnly(42);
    renderBar();

    expect(await screen.findByText("42 titles")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("media_count");
  });

  it("uses the singular form for one title", async () => {
    countOnly(1);
    renderBar();

    expect(await screen.findByText("1 title")).toBeInTheDocument();
  });

  it("falls back to zero before the count loads", async () => {
    countOnly(0);
    renderBar();

    expect(await screen.findByText("0 titles")).toBeInTheDocument();
  });
});
