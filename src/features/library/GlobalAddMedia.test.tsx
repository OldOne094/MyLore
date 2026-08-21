import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { GlobalAddMedia } from "./GlobalAddMedia";
import { ShortcutsDialog } from "@/components/shell/ShortcutsDialog";

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
    if (cmd === "media_create") return { id: "m-new" };
    throw new Error(`unexpected command ${cmd}`);
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("GlobalAddMedia", () => {
  it("opens through the palette's window event", async () => {
    const user = userEvent.setup();
    renderUI(<GlobalAddMedia />);

    window.dispatchEvent(new Event("mylore:open-add-media"));
    expect(await screen.findByRole("heading", { name: "Add a title" })).toBeInTheDocument();
    await user.type(screen.getByLabelText("Title", { exact: true }), "Dune");
  });

  it("opens with Mod+N and closes on cancel", async () => {
    const user = userEvent.setup();
    renderUI(<GlobalAddMedia />);

    await user.keyboard("{Control>}n{/Control}");
    expect(await screen.findByText("Add a title")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByText("Add a title")).not.toBeInTheDocument();
  });
});

describe("ShortcutsDialog", () => {
  it("lists the complete shortcut map when opened with ?", async () => {
    const user = userEvent.setup();
    renderUI(<ShortcutsDialog />);

    await user.keyboard("?");
    expect(await screen.findByText("Keyboard shortcuts")).toBeInTheDocument();
    expect(screen.getByText("Open command palette")).toBeInTheDocument();
    expect(screen.getByText("Quick capture (mark progress)")).toBeInTheDocument();
    expect(screen.getByText("Add title")).toBeInTheDocument();
    expect(screen.getByText("Show this help")).toBeInTheDocument();
    // Platform-aware hints are rendered as kbd chips.
    expect(screen.getAllByText(/Ctrl\+K|⌘K/).length).toBeGreaterThan(0);
  });

  it("opens through the palette's window event", async () => {
    renderUI(<ShortcutsDialog />);

    window.dispatchEvent(new Event("mylore:open-shortcuts"));
    expect(await screen.findByText("Keyboard shortcuts")).toBeInTheDocument();
  });
});
