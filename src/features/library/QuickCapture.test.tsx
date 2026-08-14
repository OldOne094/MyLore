import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { QuickCapture } from "./QuickCapture";
import { listItem } from "./testFixtures";
import type { ContentNode } from "@/api";

const ANIME = listItem({
  id: "m-1",
  content_type: "anime",
  title: "Steins;Gate",
  progress: { percent: 40, completed: 2, total: 5, next_label: "E3", next_node_id: "e3" },
});

function episode(id: string, position: number, state: string | null, number: string): ContentNode {
  return {
    id,
    kind: "episode",
    position,
    number,
    title: null,
    release_date: null,
    duration_min: 24,
    page_count: null,
    synopsis: null,
    is_special: false,
    state,
    children: [],
  };
}

const EPISODES: ContentNode[] = [
  episode("e1", 1, "watched", "1"),
  episode("e2", 2, "watched", "2"),
  episode("e3", 3, null, "3"),
  episode("e4", 4, null, "4"),
];

const ALL_CONSUMED: ContentNode[] = [
  episode("e1", 1, "watched", "1"),
  episode("e2", 2, "watched", "2"),
];

function mockInvoke(nodes: ContentNode[] = EPISODES) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "media_search") return Promise.resolve([ANIME]);
    if (cmd === "media_nodes") return Promise.resolve(nodes);
    if (cmd === "node_progress_next")
      return Promise.resolve({
        media_id: "m-1",
        summary: { percent: 60, completed: 3, total: 5, next_label: "E4", next_node_id: "e4" },
      });
    if (cmd === "node_progress_range") return Promise.resolve(["e3", "e4"]);
    return Promise.resolve(undefined);
  });
}

function renderCapture() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <QuickCapture />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

function openCapture() {
  renderCapture();
  window.dispatchEvent(new Event("mylore:open-quick-capture"));
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("QuickCapture", () => {
  it("is closed by default and opens on the palette event", async () => {
    openCapture();
    expect(await screen.findByRole("combobox")).toBeInTheDocument();
  });

  it("opens with the Mod+Enter shortcut", async () => {
    renderCapture();
    fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
    expect(await screen.findByRole("combobox")).toBeInTheDocument();
  });

  it("type-ahead searches the library and picks a title", async () => {
    mockInvoke();
    const user = userEvent.setup();
    openCapture();
    const input = await screen.findByRole("combobox");

    await user.type(input, "gate");
    expect(await screen.findByText("Steins;Gate")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("media_search", { query: "gate" });

    await user.click(screen.getByText("Steins;Gate"));
    expect(await screen.findByRole("button", { name: /Mark next done/ })).toBeInTheDocument();
  });

  it("marks the next unit of the selected title", async () => {
    mockInvoke();
    const user = userEvent.setup();
    openCapture();
    const input = await screen.findByRole("combobox");
    await user.type(input, "gate");
    await user.click(await screen.findByText("Steins;Gate"));

    await user.click(await screen.findByRole("button", { name: /Mark next done/ }));
    expect(invoke).toHaveBeenCalledWith("node_progress_next", { media_id: "m-1" });
    expect(await screen.findByText("Marked E3 done")).toBeInTheDocument();
  });

  it("marks up to N units through the range command", async () => {
    mockInvoke();
    const user = userEvent.setup();
    openCapture();
    const input = await screen.findByRole("combobox");
    await user.type(input, "gate");
    await user.click(await screen.findByText("Steins;Gate"));

    await user.click(await screen.findByRole("button", { name: "Mark up to 2" }));
    expect(invoke).toHaveBeenCalledWith("node_progress_range", {
      media_id: "m-1",
      from_id: "e3",
      to_id: "e4",
      node_state: "watched",
    });
    expect(await screen.findByText("Marked 2 units")).toBeInTheDocument();
  });

  it("shows the all-caught-up state when every unit is consumed", async () => {
    mockInvoke(ALL_CONSUMED);
    const user = userEvent.setup();
    openCapture();
    const input = await screen.findByRole("combobox");
    await user.type(input, "gate");
    await user.click(await screen.findByText("Steins;Gate"));

    expect(await screen.findByText("Everything is caught up")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Mark next done/ })).not.toBeInTheDocument();
  });
});
