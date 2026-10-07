import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import type { MediaDetail } from "./api";
import { EditMediaDialog } from "./EditMediaDialog";

/* MISSION-161 — the edit surface. The dialog must send the *full* current
   values (so clearing a field is explicit), and a field the user changed is
   pinned by the backend; the pinned list is what makes a later provider refresh
   keep the edit instead of replacing it. */

const MEDIA: MediaDetail = {
  id: "m-1",
  content_type: "novel",
  format: "light_novel",
  title_main: "Sword of the Dawn",
  title_original: null,
  synopsis: "A blade that learns to dream.",
  pub_status: "ongoing",
  start_date: null,
  end_date: null,
  release_year: 2020,
  language: "ja",
  country: "JP",
  content_rating: null,
  pages: 320,
  duration_min: null,
  ep_count: null,
  ch_count: null,
  cover_asset_id: null,
  banner_asset_id: null,
  provider: "anilist",
  provider_url: "https://anilist.co/manga/1",
  metadata_refreshed_at: null,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
  alt_titles: [],
  people: [],
  genres: ["fantasy"],
  tags: [],
  external_ids: [],
  relations: [],
};

function mockWorld(overrides: string[] = []) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "media_overrides":
        return Promise.resolve(overrides);
      case "media_facets":
        return Promise.resolve({
          formats: [],
          genres: [{ id: "fantasy", name: "Fantasy" }],
          tags: [],
          years: [],
        });
      case "media_update":
        return Promise.resolve("m-1");
      default:
        return Promise.resolve(undefined);
    }
  });
}

function renderDialog() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <EditMediaDialog media={MEDIA} onOpenChange={() => {}} />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("EditMediaDialog", () => {
  it("opens on the current values and saves the edit", async () => {
    mockWorld();
    renderDialog();

    // Prefilled, with genre ids turned into names once the facet list lands.
    expect(await screen.findByLabelText("Title")).toHaveValue("Sword of the Dawn");
    await waitFor(() => expect(screen.getByLabelText("Genres")).toHaveValue("Fantasy"));
    expect(screen.getByLabelText("Release year")).toHaveValue("2020");

    await userEvent.clear(screen.getByLabelText("Title"));
    await userEvent.type(screen.getByLabelText("Title"), "Sword of the Dusk");
    await userEvent.clear(screen.getByLabelText("Synopsis"));
    await userEvent.click(screen.getByRole("button", { name: "Save changes" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "media_update",
        expect.objectContaining({
          id: "m-1",
          title: "Sword of the Dusk",
          pubStatus: "ongoing",
          // Cleared on purpose, so it is sent as null rather than dropped.
          synopsis: null,
          genres: ["Fantasy"],
          unpin: [],
        }),
      );
    });
    expect(await screen.findByText("Changes saved")).toBeInTheDocument();
  });

  it("releases a pinned field so the provider may own it again", async () => {
    mockWorld(["title_main"]);
    renderDialog();

    // The pin is visible, and one tick hands the field back.
    expect(await screen.findByText("Kept from your edit")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("checkbox", { name: "Title" }));
    await userEvent.click(screen.getByRole("button", { name: "Save changes" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "media_update",
        expect.objectContaining({ unpin: ["title_main"] }),
      );
    });
  });

  it("refuses a blank title without calling the command", async () => {
    mockWorld();
    renderDialog();

    await userEvent.clear(await screen.findByLabelText("Title"));
    await userEvent.click(screen.getByRole("button", { name: "Save changes" }));

    expect(await screen.findByText("This field is required.")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("media_update", expect.anything());
  });

  it("sets a cover through the image pipeline (MISSION-167)", async () => {
    mockWorld();
    renderDialog();

    const field = await screen.findByLabelText("Cover image URL");
    await userEvent.type(field, "https://img.test/a.jpg");
    await userEvent.click(screen.getByRole("button", { name: "Set cover" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("media_set_cover", {
        id: "m-1",
        url: "https://img.test/a.jpg",
      });
    });
    expect(await screen.findByText("Cover updated")).toBeInTheDocument();
  });
});
