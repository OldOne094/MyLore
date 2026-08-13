import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";
import type { InvokeArgs } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { NodeTree } from "./NodeTree";
import type { ContentNode } from "@/api";

const VOLUMES: ContentNode[] = [
  {
    id: "v1",
    kind: "volume",
    position: 1,
    number: "1",
    title: "The Beginning",
    release_date: null,
    duration_min: null,
    page_count: 320,
    synopsis: null,
    is_special: false,
    state: null,
    children: [
      {
        id: "c1",
        kind: "chapter",
        position: 1,
        number: "1",
        title: null,
        release_date: null,
        duration_min: null,
        page_count: 24,
        synopsis: null,
        is_special: false,
        state: null,
        children: [],
      },
      {
        id: "c2",
        kind: "chapter",
        position: 2,
        number: "2",
        title: "The Road Home",
        release_date: null,
        duration_min: null,
        page_count: 25,
        synopsis: null,
        is_special: true,
        state: null,
        children: [],
      },
    ],
  },
  {
    id: "v2",
    kind: "volume",
    position: 2,
    number: "2",
    title: null,
    release_date: null,
    duration_min: null,
    page_count: 300,
    synopsis: null,
    is_special: false,
    state: null,
    children: [],
  },
];

function renderTree(contentType = "manga") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <NodeTree mediaId="m-1" mediaTitle="Sword of the Dawn" contentType={contentType} />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

/** Mocks the three IPC commands NodeTree depends on, defaulting the progress
    writes to success. Ranges report the from/to ids as the affected set. */
function mockTreeApi(options: { rangeIds?: string[] } = {}) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: InvokeArgs) => {
    const params = (args ?? {}) as Record<string, string>;
    if (cmd === "media_nodes") return Promise.resolve(VOLUMES);
    if (cmd === "node_progress_set") return Promise.resolve();
    if (cmd === "node_progress_range") {
      const ids = options.rangeIds ?? [params.from_id, params.to_id];
      return Promise.resolve(ids);
    }
    return Promise.reject(new Error(`unhandled cmd ${cmd}`));
  });
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("NodeTree", () => {
  it("renders roots expanded with tree semantics and labels", async () => {
    vi.mocked(invoke).mockResolvedValue(VOLUMES);
    renderTree();

    const tree = await screen.findByRole("tree", { name: "Sword of the Dawn content tree" });
    const volume = screen.getByRole("treeitem", { name: "Volume 1 · The Beginning" });
    expect(volume).toHaveAttribute("aria-level", "1");
    expect(volume).toHaveAttribute("aria-expanded", "true");
    expect(volume).toHaveAttribute("aria-posinset", "1");
    expect(screen.getByRole("treeitem", { name: "Chapter 1" })).toHaveAttribute("aria-level", "2");
    expect(screen.getByRole("treeitem", { name: "Chapter 2 · The Road Home" })).toBeInTheDocument();

    const leaf = screen.getByRole("treeitem", { name: "Volume 2" });
    expect(leaf).not.toHaveAttribute("aria-expanded");
    expect(tree).toBeInTheDocument();
  });

  it("shows page counts and the special badge", async () => {
    vi.mocked(invoke).mockResolvedValue(VOLUMES);
    renderTree();
    await screen.findByRole("tree", { name: "Sword of the Dawn content tree" });

    const chapter1 = screen.getByRole("treeitem", { name: "Chapter 1" });
    expect(within(chapter1).getByText("24 pages")).toBeInTheDocument();
    expect(within(chapter1).queryByText("Special")).not.toBeInTheDocument();

    const chapter2 = screen.getByRole("treeitem", { name: "Chapter 2 · The Road Home" });
    expect(within(chapter2).getByText("Special")).toBeInTheDocument();
    expect(within(chapter2).getByText("25 pages")).toBeInTheDocument();
    expect(
      within(screen.getByRole("treeitem", { name: "Volume 1 · The Beginning" })).getByText(
        "320 pages",
      ),
    ).toBeInTheDocument();
  });

  it("collapses and re-expands children through the toggle button", async () => {
    vi.mocked(invoke).mockResolvedValue(VOLUMES);
    renderTree();
    const volume = await screen.findByRole("treeitem", { name: "Volume 1 · The Beginning" });

    await userEvent.click(
      screen.getByRole("button", { name: "Hide children of Volume 1 · The Beginning" }),
    );
    expect(volume).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("treeitem", { name: "Chapter 1" })).not.toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", {
        name: "Show children of Volume 1 · The Beginning",
      }),
    );
    expect(volume).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByRole("treeitem", { name: "Chapter 1" })).toBeInTheDocument();
  });

  it("shows an empty state when the media has no nodes", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    renderTree();
    expect(await screen.findByText("No content structure")).toBeInTheDocument();
  });

  it("shows an error state with retry", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("boom").mockResolvedValueOnce(VOLUMES);
    renderTree();

    await userEvent.click(await screen.findByRole("button", { name: "Retry" }));
    expect(
      await screen.findByRole("treeitem", { name: "Volume 1 · The Beginning" }),
    ).toBeInTheDocument();
  });

  it("expands, collapses and moves focus with the arrow keys", async () => {
    vi.mocked(invoke).mockResolvedValue(VOLUMES);
    renderTree();

    const volume = await screen.findByRole("treeitem", { name: "Volume 1 · The Beginning" });
    await userEvent.click(
      screen.getByRole("button", { name: "Hide children of Volume 1 · The Beginning" }),
    );
    volume.focus();

    await userEvent.keyboard("{ArrowRight}");
    expect(volume).toHaveAttribute("aria-expanded", "true");

    const chapter1 = await screen.findByRole("treeitem", { name: "Chapter 1" });
    await userEvent.keyboard("{ArrowDown}");
    expect(chapter1).toHaveFocus();

    await userEvent.keyboard("{ArrowUp}");
    expect(volume).toHaveFocus();

    await userEvent.keyboard("{ArrowLeft}");
    expect(volume).toHaveAttribute("aria-expanded", "false");
  });

  it("marks a single node read and unread through its checkbox", async () => {
    mockTreeApi();
    renderTree();

    const checkbox = await screen.findByRole("checkbox", { name: "Mark Chapter 1 as read" });
    await userEvent.click(checkbox);
    expect(invoke).toHaveBeenCalledWith("node_progress_set", {
      node_id: "c1",
      node_state: "read",
    });
    expect(screen.getByRole("checkbox", { name: "Mark Chapter 1 as unread" })).toBeChecked();

    await userEvent.click(screen.getByRole("checkbox", { name: "Mark Chapter 1 as unread" }));
    expect(invoke).toHaveBeenCalledWith("node_progress_set", {
      node_id: "c1",
      node_state: "unread",
    });
    expect(screen.getByRole("checkbox", { name: "Mark Chapter 1 as read" })).not.toBeChecked();
  });

  it("marks a whole range with shift-click and reconciles with the affected ids", async () => {
    mockTreeApi({ rangeIds: ["v1", "c1", "c2", "v2"] });
    renderTree();

    const volume1 = await screen.findByRole("checkbox", {
      name: "Mark Volume 1 · The Beginning as read",
    });
    await userEvent.click(volume1);
    fireEvent.click(screen.getByRole("checkbox", { name: "Mark Volume 2 as read" }), {
      shiftKey: true,
    });

    expect(invoke).toHaveBeenCalledWith("node_progress_range", {
      media_id: "m-1",
      from_id: "v1",
      to_id: "v2",
      node_state: "read",
    });
    expect(screen.getByRole("checkbox", { name: "Mark Chapter 1 as unread" })).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: "Mark Chapter 2 · The Road Home as unread" }),
    ).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Mark Volume 2 as unread" })).toBeChecked();
  });

  it("skips a node and restores it with the skip button", async () => {
    mockTreeApi();
    renderTree();

    const chapter1 = await screen.findByRole("treeitem", { name: "Chapter 1" });
    await userEvent.click(
      within(chapter1).getByRole("button", { name: "Mark Chapter 1 as skipped" }),
    );
    expect(invoke).toHaveBeenCalledWith("node_progress_set", {
      node_id: "c1",
      node_state: "skipped",
    });

    await userEvent.click(screen.getByRole("button", { name: "Mark Chapter 1 as unread" }));
    expect(invoke).toHaveBeenCalledWith("node_progress_set", {
      node_id: "c1",
      node_state: "unread",
    });
  });

  it("uses watched as the consuming state for anime and tv", async () => {
    mockTreeApi();
    renderTree("anime");
    const checkbox = await screen.findByRole("checkbox", { name: "Mark Chapter 1 as watched" });
    await userEvent.click(checkbox);
    expect(invoke).toHaveBeenCalledWith("node_progress_set", {
      node_id: "c1",
      node_state: "watched",
    });
  });

  it("shows completed nodes with their state already applied", async () => {
    const tree = VOLUMES.map((volume) => ({
      ...volume,
      state: volume.id === "v1" ? "read" : null,
    }));
    vi.mocked(invoke).mockResolvedValue(tree);
    renderTree();
    await screen.findByRole("tree", { name: "Sword of the Dawn content tree" });
    expect(
      screen.getByRole("checkbox", { name: "Mark Volume 1 · The Beginning as unread" }),
    ).toBeChecked();
  });
});
