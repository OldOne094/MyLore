/* MISSION-155 — the task centre. Before it, a background task was only visible
   from the dialog that spawned it: close the dialog and running work vanished,
   while `task_list` had no caller at all. */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import i18n from "@/i18n";
import { TaskCenter } from "./TaskCenter";

void i18n;

const task_list = vi.fn();
const task_cancel = vi.fn();

vi.mock("@/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/api")>();
  return {
    ...actual,
    task_list: (...args: unknown[]) => task_list(...args),
    task_cancel: (...args: unknown[]) => task_cancel(...args),
    listenTaskChanged: vi.fn().mockResolvedValue(() => {}),
  };
});

function snapshot(overrides: Record<string, unknown>) {
  return {
    id: "t-1",
    kind: "import_file",
    title: "Import JSON file",
    state: "running",
    progress: 40,
    message: "Importing 4/10 titles",
    error: null,
    result: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function renderCenter() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    (
      <QueryClientProvider client={client}>
        <TaskCenter />
      </QueryClientProvider>
    ) as ReactNode,
  );
}

describe("TaskCenter", () => {
  beforeEach(() => {
    task_list.mockReset();
    task_cancel.mockReset();
  });

  it("shows every task and cancels a live one", async () => {
    task_list.mockResolvedValue([
      snapshot({ id: "t-live", title: "Import JSON file", state: "running", progress: 40 }),
      snapshot({
        id: "t-done",
        title: "Create library backup",
        state: "success",
        progress: 100,
        message: null,
      }),
    ]);
    task_cancel.mockResolvedValue(snapshot({ id: "t-live", state: "cancelled" }));

    renderCenter();
    await userEvent.click(screen.getByRole("button", { name: /tasks/i }));

    expect(await screen.findByText("Import JSON file")).toBeInTheDocument();
    expect(screen.getByText("Create library backup")).toBeInTheDocument();
    expect(screen.getByText("Importing 4/10 titles")).toBeInTheDocument();

    // Only the live task can be cancelled — a finished one has no button.
    const cancels = screen.getAllByRole("button", { name: "Cancel" });
    expect(cancels).toHaveLength(1);
    await userEvent.click(cancels[0]);
    await waitFor(() => expect(task_cancel).toHaveBeenCalledWith({ id: "t-live" }));
  });

  it("says so when nothing is running", async () => {
    task_list.mockResolvedValue([]);
    renderCenter();
    await userEvent.click(screen.getByRole("button", { name: /tasks/i }));
    expect(await screen.findByText(/Nothing running/)).toBeInTheDocument();
  });

  it("surfaces a failed task's error", async () => {
    task_list.mockResolvedValue([
      snapshot({
        id: "t-bad",
        title: "Restore library backup",
        state: "failed",
        error: "unknown task",
      }),
    ]);
    renderCenter();
    await userEvent.click(screen.getByRole("button", { name: /tasks/i }));
    expect(await screen.findByText("unknown task")).toBeInTheDocument();
  });
});
