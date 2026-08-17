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
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
  emit: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { ExportSection } from "./ExportSection";
import type { ExportReport, TaskSnapshot } from "@/api";

const TASK_ID = "t-export-1";
let currentTask: TaskSnapshot;
let taskListener: ((snapshot: TaskSnapshot) => void) | undefined;

const REPORT: ExportReport = {
  format: "json",
  total: 2,
  path: "C:\\Users\\me\\Documents\\mylore-library.json",
};

function makeSnapshot(
  state: TaskSnapshot["state"],
  overrides: Partial<TaskSnapshot> = {},
): TaskSnapshot {
  return {
    id: TASK_ID,
    kind: "export_file",
    title: "Export json library",
    state,
    progress: null,
    message: null,
    error: null,
    result: null,
    created_at: "2026-08-16T00:00:00Z",
    updated_at: "2026-08-16T00:00:00Z",
    ...overrides,
  };
}

function emitTask(next: Partial<TaskSnapshot>) {
  currentTask = { ...currentTask, ...next, id: TASK_ID };
  taskListener?.(currentTask);
}

function renderSection() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <ExportSection />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

async function openDialog(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Export" }));
  return screen.getByRole("dialog");
}

beforeEach(() => {
  taskListener = undefined;
  currentTask = makeSnapshot("running", { progress: 0, message: "Preparing export…" });
  vi.mocked(listen).mockImplementation(((
    _event: string,
    handler: (event: { payload: TaskSnapshot }) => void,
  ) => {
    taskListener = (snapshot) => handler({ payload: snapshot });
    return Promise.resolve(() => undefined);
  }) as typeof listen);
  vi.mocked(save).mockResolvedValue("C:\\Users\\me\\Documents\\mylore-library.json");
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "export_media":
        return currentTask;
      case "task_get":
        return currentTask;
      default:
        throw new Error(`unexpected command ${cmd}`);
    }
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
  vi.mocked(save).mockReset();
  taskListener = undefined;
  await i18n.changeLanguage("en");
});

describe("ExportSection", () => {
  it("exports the library as JSON through the save dialog and shows the report", async () => {
    const user = userEvent.setup();
    renderSection();
    const dialog = await openDialog(user);

    await user.click(within(dialog).getByRole("button", { name: "Export" }));

    expect(save).toHaveBeenCalledWith({
      title: "mylore",
      defaultPath: "mylore-library.json",
      filters: [{ name: "Export", extensions: ["json"] }],
    });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("export_media", {
        format: "json",
        path: "C:\\Users\\me\\Documents\\mylore-library.json",
      });
    });

    expect(await screen.findByRole("status")).toBeInTheDocument();
    expect(screen.getByText("Exporting…")).toBeInTheDocument();

    emitTask({ state: "running", progress: 50, message: "Exporting 1/2 titles" });
    expect(await screen.findByText("Exporting 1/2 titles")).toBeInTheDocument();

    emitTask({ state: "success", progress: 100, result: REPORT });
    expect(await screen.findByText("Export finished")).toBeInTheDocument();
    expect(
      screen.getByText("Exported 2 titles · Saved to mylore-library.json"),
    ).toBeInTheDocument();
  });

  it("switches the format to CSV and passes it through", async () => {
    const user = userEvent.setup();
    renderSection();
    const dialog = await openDialog(user);

    await user.click(within(dialog).getByRole("button", { name: "CSV" }));
    await user.click(within(dialog).getByRole("button", { name: "Export" }));

    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: "mylore-library.csv",
        filters: [{ name: "Export", extensions: ["csv"] }],
      }),
    );
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("export_media", {
        format: "csv",
        path: "C:\\Users\\me\\Documents\\mylore-library.json",
      });
    });
  });

  it("does nothing when the save dialog is cancelled", async () => {
    vi.mocked(save).mockResolvedValue(null);
    const user = userEvent.setup();
    renderSection();
    const dialog = await openDialog(user);

    await user.click(within(dialog).getByRole("button", { name: "Export" }));

    expect(save).toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalledWith("export_media", expect.anything());
  });

  it("shows the error panel when the export task fails", async () => {
    const user = userEvent.setup();
    renderSection();
    const dialog = await openDialog(user);

    await user.click(within(dialog).getByRole("button", { name: "Export" }));

    emitTask({ state: "failed", error: "Disk full" });
    expect(await screen.findByText("Couldn't export the library")).toBeInTheDocument();
    expect(screen.getByText("Disk full")).toBeInTheDocument();
  });

  it("shows the cancelled panel when the export task is cancelled", async () => {
    const user = userEvent.setup();
    renderSection();
    const dialog = await openDialog(user);

    await user.click(within(dialog).getByRole("button", { name: "Export" }));

    emitTask({ state: "cancelled" });
    expect(await screen.findByText("Export cancelled")).toBeInTheDocument();
  });
});
