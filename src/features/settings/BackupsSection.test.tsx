import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";
import type { TaskSnapshot } from "@/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
  emit: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BackupsSection } from "./BackupsSection";

const NEWEST = "mylore-20260820-120000-aaaaaa.mylore";
const OLDER = "mylore-20260819-120000-bbbbbb.mylore";
const NEWEST_PATH = `C:\\data\\backups\\${NEWEST}`;

const ARCHIVES = [
  {
    file_name: NEWEST,
    path: NEWEST_PATH,
    size_bytes: 2048,
    created_at: "20260820120000",
  },
  {
    file_name: OLDER,
    path: `C:\\data\\backups\\${OLDER}`,
    size_bytes: 1024,
    created_at: "20260819120000",
  },
];

const PREFS = { auto_enabled: false, interval_hours: 24, keep_count: 10 };

function makeSnapshot(state: TaskSnapshot["state"], result: unknown): TaskSnapshot {
  return {
    id: "t-backup-1",
    kind: "backup",
    title: "Create library backup",
    state,
    progress: null,
    message: null,
    error: null,
    result,
    created_at: "2026-08-21T00:00:00Z",
    updated_at: "2026-08-21T00:00:00Z",
  };
}

// Whatever the last spawned task returned; `task_get` serves it so the
// section's task-following hooks see the terminal state.
let lastTask: TaskSnapshot;

function renderSection() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <BackupsSection />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  lastTask = makeSnapshot("success", null);
  vi.mocked(listen).mockImplementation((() =>
    Promise.resolve(() => undefined)) as unknown as typeof listen);
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    switch (cmd) {
      case "backup_list":
        return ARCHIVES;
      case "backup_prefs_get":
        return PREFS;
      case "backup_prefs_set":
        return args;
      case "backup_validate":
        return Promise.reject(new Error("broken archive"));
      case "backup_delete":
        return undefined;
      case "backup_create":
        lastTask = makeSnapshot("success", {
          path: NEWEST_PATH.replace(NEWEST, "mylore-20260821-000000-cccccc.mylore"),
          size_bytes: 4096,
          media_count: 3,
          asset_count: 0,
        });
        return lastTask;
      case "backup_restore":
        lastTask = makeSnapshot("success", {
          quarantined_to: "C:\\data\\quarantine-x",
          restart_required: true,
        });
        return lastTask;
      case "task_get":
        return lastTask;
      default:
        throw new Error(`unexpected command ${cmd}`);
    }
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
  await i18n.changeLanguage("en");
});

describe("BackupsSection", () => {
  it("shows a calm empty state when no archives exist (MISSION-091)", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "backup_list") return [];
      if (cmd === "backup_prefs_get") return PREFS;
      throw new Error(`unexpected command ${cmd}`);
    });
    renderSection();

    expect(await screen.findByText("No backups yet.")).toBeInTheDocument();
  });

  it("lists archives newest first with date and size", async () => {
    renderSection();

    expect(await screen.findByText(NEWEST)).toBeInTheDocument();
    expect(screen.getByText(OLDER)).toBeInTheDocument();
    expect(screen.getByText("2026-08-20 12:00")).toBeInTheDocument();
    expect(screen.getByText("2 KB")).toBeInTheDocument();
    expect(screen.getByText("1 KB")).toBeInTheDocument();
  });

  it("marks a broken archive after a failed check", async () => {
    const user = userEvent.setup();
    renderSection();
    await screen.findByText(NEWEST);

    const row = screen.getByText(NEWEST).closest("li");
    expect(row).not.toBeNull();
    await user.click(within(row!).getByRole("button", { name: "Check" }));

    expect(await within(row!).findByText("Broken")).toBeInTheDocument();
  });

  it("deletes an archive through the two-step confirm", async () => {
    const user = userEvent.setup();
    renderSection();
    await screen.findByText(OLDER);

    const row = screen.getByText(OLDER).closest("li");
    expect(row).not.toBeNull();
    await user.click(within(row!).getByRole("button", { name: "Delete" }));
    await user.click(within(row!).getByRole("button", { name: "Really delete?" }));

    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("backup_delete", {
        path: `C:\\data\\backups\\${OLDER}`,
      }),
    );
  });

  it("restores through the guarded dialog and asks for a restart", async () => {
    const user = userEvent.setup();
    renderSection();
    await screen.findByText(NEWEST);

    await user.click((await screen.findAllByRole("button", { name: "Restore" }))[0]);
    const dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Restore" }));

    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("backup_restore", { path: NEWEST_PATH }),
    );
    expect(await screen.findByText(/restart MyLore/i)).toBeInTheDocument();
  });

  it("creates a backup and reports the new archive", async () => {
    const user = userEvent.setup();
    renderSection();
    await screen.findByText(NEWEST);

    await user.click(screen.getByRole("button", { name: "Back up now" }));

    expect(
      await screen.findByText(/mylore-20260821-000000-cccccc\.mylore created/),
    ).toBeInTheDocument();
  });
});
