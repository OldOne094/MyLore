import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { HealthGate, RecoveryScreen } from "./RecoveryScreen";

const ARCHIVES = [
  {
    file_name: "mylore-20260820-120000-aaaaaa.mylore",
    path: "C:\\data\\backups\\mylore-20260820-120000-aaaaaa.mylore",
    size_bytes: 2048,
    created_at: "20260820120000",
  },
];

function renderUI(ui: React.ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>{ui}</ToastProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "app_health":
        return { database_ok: false };
      case "backup_list":
        return ARCHIVES;
      case "recover_restore":
        return { quarantined_to: "C:\\data\\quarantine-corrupt-x", restart_required: true };
      case "recover_start_fresh":
        return { quarantined_to: "C:\\data\\quarantine-corrupt-y", restart_required: true };
      default:
        throw new Error(`unexpected command ${cmd}`);
    }
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("RecoveryScreen", () => {
  it("offers both exits over the archive list", async () => {
    renderUI(<RecoveryScreen />);

    expect(await screen.findByText("Database needs attention")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose archive…" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start fresh" })).toBeInTheDocument();
    expect(await screen.findByText("mylore-20260820-120000-aaaaaa.mylore")).toBeInTheDocument();
  });

  it("restores an archive and reports the quarantine + restart note", async () => {
    const user = userEvent.setup();
    renderUI(<RecoveryScreen />);

    const row = (await screen.findByText("mylore-20260820-120000-aaaaaa.mylore")).closest("li");
    expect(row).not.toBeNull();
    await user.click(within(row!).getByRole("button", { name: "Restore" }));

    expect(await screen.findByText(/Close and reopen MyLore/i)).toBeInTheDocument();
    expect(screen.getByText(/quarantine-corrupt-x/)).toBeInTheDocument();
  });

  it("starts fresh after a two-step confirm", async () => {
    const user = userEvent.setup();
    renderUI(<RecoveryScreen />);

    await user.click(screen.getByRole("button", { name: "Start fresh" }));
    await user.click(screen.getByRole("button", { name: "Start over with an empty library?" }));

    expect(await screen.findByText(/Close and reopen MyLore/i)).toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("recover_start_fresh");
  });
});

describe("HealthGate", () => {
  it("renders the recovery screen instead of the shell when unhealthy", async () => {
    renderUI(
      <HealthGate>
        <div>SHELL</div>
      </HealthGate>,
    );
    expect(await screen.findByText("Database needs attention")).toBeInTheDocument();
    expect(screen.queryByText("SHELL")).not.toBeInTheDocument();
  });

  it("passes the shell through when the database is healthy", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "app_health") return { database_ok: true };
      throw new Error(`unexpected command ${cmd}`);
    });
    renderUI(
      <HealthGate>
        <div>SHELL</div>
      </HealthGate>,
    );
    expect(await screen.findByText("SHELL")).toBeInTheDocument();
  });
});
