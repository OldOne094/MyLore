import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { GroupsPage } from "./GroupsPage";

const PREFS = { enabled: true, member_id: "m-me", display_name: "Me" };
const GROUP = {
  id: "g-1",
  name: "Tuesday book club",
  owner_id: "m-me",
  epoch: 0,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
  members: [
    { member_id: "m-me", display_name: "Me", role: "owner", joined_at: "2026-01-01T00:00:00Z" },
  ],
  shelf_count: 0,
  note_count: 0,
};

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={["/groups"]}>
          <Routes>
            <Route path="/groups" element={<GroupsPage />} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("GroupsPage", () => {
  it("asks for opt-in and states what leaves the device before anything is enabled", async () => {
    vi.mocked(invoke).mockResolvedValue({ enabled: false, member_id: "m-me", display_name: "" });
    renderPage();

    expect(await screen.findByText("What leaves this device")).toBeInTheDocument();
    expect(screen.getByText("A relay can still see")).toBeInTheDocument();
    // No group surface exists until the user opts in.
    expect(screen.queryByRole("button", { name: "New group" })).not.toBeInTheDocument();
  });

  it("enables reading groups with the chosen name", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "reading_group_prefs_set") return Promise.resolve(PREFS);
      return Promise.resolve({ enabled: false, member_id: "m-me", display_name: "" });
    });
    renderPage();

    await userEvent.type(await screen.findByLabelText("Your name in groups"), "Me");
    await userEvent.click(screen.getByRole("button", { name: "Turn on reading groups" }));

    expect(invoke).toHaveBeenCalledWith("reading_group_prefs_set", {
      enabled: true,
      displayName: "Me",
    });
  });

  it("lists groups with their member count", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "reading_group_prefs_get") return Promise.resolve(PREFS);
      if (cmd === "reading_group_list") return Promise.resolve([GROUP]);
      return Promise.resolve(undefined);
    });
    renderPage();

    expect(await screen.findByText("Tuesday book club")).toBeInTheDocument();
    expect(screen.getByText("1 member")).toBeInTheDocument();
  });

  it("shows an empty state with a way to start", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "reading_group_prefs_get") return Promise.resolve(PREFS);
      if (cmd === "reading_group_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    renderPage();

    expect(await screen.findByText("No groups yet")).toBeInTheDocument();
    // Both the header action and the empty state offer a way in.
    expect(screen.getAllByRole("button", { name: "New group" }).length).toBeGreaterThan(0);
  });

  it("recovers from a failed load", async () => {
    let failing = true;
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "reading_group_prefs_get") {
        if (failing) {
          failing = false;
          return Promise.reject("boom");
        }
        return Promise.resolve({ enabled: false, member_id: "m-me", display_name: "" });
      }
      return Promise.resolve(undefined);
    });
    renderPage();

    const retry = await screen.findByRole("button", { name: "Retry" });
    await userEvent.click(retry);
    expect(await screen.findByText("What leaves this device")).toBeInTheDocument();
  });
});
