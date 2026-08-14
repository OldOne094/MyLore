import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { MediaCard } from "./MediaCard";
import { MediaRow } from "./MediaRow";
import { listItem } from "./testFixtures";

const NEXT = {
  percent: 40,
  completed: 2,
  total: 5,
  next_label: "E3",
  next_node_id: "e3",
};

const DONE = {
  percent: 100,
  completed: 5,
  total: 5,
  next_label: null,
  next_node_id: null,
};

function wrap(children: React.ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter>{children}</MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("NextUnitButton", () => {
  it("renders a labeled pill on the card with the next unit", () => {
    wrap(<MediaCard item={listItem({ content_type: "anime", progress: NEXT })} />);
    expect(screen.getByRole("button", { name: "Mark E3 as watched" })).toHaveTextContent("E3");
  });

  it("marks the next unit through node_progress_next", async () => {
    const user = userEvent.setup();
    wrap(<MediaCard item={listItem({ content_type: "anime", progress: NEXT })} />);
    vi.mocked(invoke).mockResolvedValue({
      media_id: "m-1",
      summary: { percent: 60, completed: 3, total: 5, next_label: "E4", next_node_id: "e4" },
    });

    await user.click(screen.getByRole("button", { name: "Mark E3 as watched" }));
    expect(invoke).toHaveBeenCalledWith("node_progress_next", { media_id: "m-1" });
  });

  it("shows the all-caught-up info toast when nothing is left to mark", async () => {
    const user = userEvent.setup();
    wrap(<MediaCard item={listItem({ content_type: "anime", progress: NEXT })} />);
    vi.mocked(invoke).mockResolvedValue(null);

    await user.click(screen.getByRole("button", { name: "Mark E3 as watched" }));
    expect(await screen.findByText("Everything is caught up")).toBeInTheDocument();
  });

  it("hides when there is nothing left to mark", () => {
    wrap(<MediaCard item={listItem({ content_type: "anime", progress: DONE })} />);
    expect(screen.queryByRole("button", { name: /mark/i })).not.toBeInTheDocument();
  });

  it("renders an icon-only control on the list row", () => {
    wrap(<MediaRow item={listItem({ content_type: "anime", progress: NEXT })} />);
    const button = screen.getByRole("button", { name: "Mark E3 as watched" });
    expect(button).toHaveAccessibleName("Mark E3 as watched");
  });

  it("is hidden in select mode", () => {
    wrap(
      <MediaCard
        item={listItem({ content_type: "anime", progress: NEXT })}
        selectable
        selected={false}
      />,
    );
    expect(screen.queryByRole("button", { name: /mark/i })).not.toBeInTheDocument();
  });
});
