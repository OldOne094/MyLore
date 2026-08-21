import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { MergeDialog } from "./MergeDialog";

const ID_A = "m-aaa";
const ID_B = "m-bbb";

function renderDialog(onMerged: () => void) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MergeDialog ids={[ID_A, ID_B]} open onClose={() => undefined} onMerged={onMerged} />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    const payload = args as Record<string, unknown> | undefined;
    switch (cmd) {
      case "media_get":
        return payload?.id === ID_A
          ? { id: ID_A, title_main: "Fairy Tail" }
          : { id: ID_B, title_main: "Fairy Tail (Duplicate)" };
      case "merge_plan":
        return {
          survivor_id: payload?.survivor_id,
          duplicate_id: payload?.duplicate_id,
          survivor_title: "Fairy Tail",
          duplicate_title: "Fairy Tail (Duplicate)",
          merged_title: "Fairy Tail",
          conflicts: [
            { field: "synopsis", survivor: "one", duplicate: "two" },
            { field: "release_year", survivor: "2009", duplicate: "2014" },
          ],
          nodes_to_move: 3,
          move_review: true,
          move_tracking: false,
          collections_to_move: 1,
        };
      case "merge_apply":
        return { trash_id: "t-merge-1" };
      default:
        throw new Error(`unexpected command ${cmd}`);
    }
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("MergeDialog", () => {
  it("previews conflicts and movements for the chosen survivor", async () => {
    const user = userEvent.setup();
    renderDialog(() => undefined);

    // Both titles load; pick the second as the survivor.
    expect(await screen.findByText("Fairy Tail")).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "Fairy Tail (Duplicate)" }));
    await user.click(screen.getByRole("button", { name: "Preview merge" }));

    const preview = await screen.findByText(/will be merged into/);
    expect(preview).toHaveTextContent('"Fairy Tail (Duplicate)"');
    expect(
      screen.getByText((_, element) => element?.textContent === "Conflicts (2)"),
    ).toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("merge_plan", {
      survivor_id: ID_B,
      duplicate_id: ID_A,
    });
  });

  it("applies the merge and reports success", async () => {
    const user = userEvent.setup();
    let merged = false;
    renderDialog(() => {
      merged = true;
    });

    await screen.findByText("Fairy Tail");
    await user.click(screen.getByRole("button", { name: "Preview merge" }));

    const dialog = screen.getByRole("dialog");
    await user.click(await within(dialog).findByRole("button", { name: "Merge" }));

    await waitFor(() => expect(merged).toBe(true));
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("merge_apply", {
      survivor_id: ID_A,
      duplicate_id: ID_B,
    });
  });
});
