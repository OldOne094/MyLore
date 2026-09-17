import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { encodeNoteId } from "@/features/groups/alignment";
import { GroupDetailPage } from "./GroupDetailPage";

const MEMBERS = [
  { member_id: "m-me", display_name: "Me", role: "owner", joined_at: "2026-01-01T00:00:00Z" },
  { member_id: "m-nour", display_name: "Nour", role: "member", joined_at: "2026-01-01T00:00:00Z" },
];

const GROUP = {
  id: "g-1",
  name: "Tuesday book club",
  owner_id: "m-me",
  epoch: 0,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
  members: MEMBERS,
  shelf_count: 2,
  note_count: 0,
};

function shelfEntry(member_id: string, progress: number) {
  return {
    group_id: "g-1",
    member_id,
    work_key: "h:berserk",
    title: "Berserk",
    content_type: "manga",
    status: "in_progress",
    progress,
    updated_at: "2026-01-02T00:00:00Z",
  };
}

/** One note from Nour, stamped the way the composer stamps notes. */
const NOUR_NOTE = encodeNoteId("m-nour", Date.parse("2026-03-04T05:06:07Z"), "a");

function mockWorld(overrides: Record<string, unknown> = {}, unsupported = false) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd in overrides) return Promise.resolve(overrides[cmd]);
    switch (cmd) {
      case "reading_group_view":
        return Promise.resolve(GROUP);
      case "reading_group_prefs_get":
        return Promise.resolve({ enabled: true, member_id: "m-me", display_name: "Me" });
      case "reading_group_shelf":
        return Promise.resolve([shelfEntry("m-me", 3), shelfEntry("m-nour", 40)]);
      case "reading_group_key_status":
        return unsupported
          ? Promise.reject(
              "this build was compiled without reading-groups p2p support (feature `p2p`)",
            )
          : Promise.resolve({ has_key: true, key_id: "abcdef123456" });
      case "reading_group_relays_get":
        return Promise.resolve({ relays: ["wss://relay.test"], pending: 0 });
      case "reading_group_note_state":
        return unsupported
          ? Promise.reject(
              "this build was compiled without reading-groups p2p support (feature `p2p`)",
            )
          : Promise.resolve([{ note_id: NOUR_NOTE, body: "The ending reveals everything." }]);
      case "reading_group_notes":
        return Promise.resolve([
          {
            id: "n-1",
            group_id: "g-1",
            work_key: "h:berserk",
            author_id: "m-me",
            body: "Local table note.",
            created_at: "2026-03-04T05:06:07Z",
            updated_at: "2026-03-04T05:06:07Z",
          },
          {
            id: "n-2",
            group_id: "g-1",
            work_key: "h:berserk",
            author_id: "m-nour",
            body: "Ahead of me, so hidden.",
            created_at: "2026-03-05T05:06:07Z",
            updated_at: "2026-03-05T05:06:07Z",
          },
        ]);
      default:
        return Promise.resolve(undefined);
    }
  });
}

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <MemoryRouter initialEntries={["/groups/g-1"]}>
          <Routes>
            <Route path="/groups/:groupId" element={<GroupDetailPage />} />
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

describe("GroupDetailPage", () => {
  it("aligns the group's shelves by work, one column per member", async () => {
    mockWorld();
    renderPage();

    expect(await screen.findByRole("button", { name: "Berserk" })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Me" })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Nour" })).toBeInTheDocument();
    // Each member's own progress, from their own shelf entry.
    expect(screen.getByText("40")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("hides a note from a member who is further along than me", async () => {
    mockWorld();
    renderPage();

    await userEvent.click(await screen.findByRole("button", { name: "Berserk" }));

    expect(await screen.findByText("Nour is at 40 — further than you")).toBeInTheDocument();
    expect(screen.queryByText("The ending reveals everything.")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Show anyway" }));
    expect(screen.getByText("The ending reveals everything.")).toBeInTheDocument();
  });

  it("posts a note stamped with my member id", async () => {
    mockWorld();
    renderPage();

    await userEvent.click(await screen.findByRole("button", { name: "Berserk" }));
    await userEvent.type(await screen.findByLabelText("Your note"), "I'm at chapter 3.");
    await userEvent.click(screen.getByRole("button", { name: "Post" }));

    expect(invoke).toHaveBeenCalledWith(
      "reading_group_note_edit",
      expect.objectContaining({
        groupId: "g-1",
        workKey: "h:berserk",
        noteId: expect.stringContaining("m-me"),
        body: "I'm at chapter 3.",
      }),
    );
  });

  it("falls back to the local notes table in a build without relay support", async () => {
    mockWorld({}, true);
    renderPage();

    // The page says what this build can and cannot do.
    expect(await screen.findByText("This device only")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Sync now" })).not.toBeInTheDocument();

    await userEvent.click(await screen.findByRole("button", { name: "Berserk" }));
    // …and the thread still works, from the local store — with the same gate:
    // my own note shows, Nour's (further along) does not.
    expect(await screen.findByText("Local table note.")).toBeInTheDocument();
    expect(screen.queryByText("Ahead of me, so hidden.")).not.toBeInTheDocument();
    expect(screen.getByText("Nour is at 40 — further than you")).toBeInTheDocument();
    expect(
      screen.getByText("Saved on this device — this build has no relay support."),
    ).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("reading_group_notes", {
      groupId: "g-1",
      workKey: "h:berserk",
    });
  });
});
