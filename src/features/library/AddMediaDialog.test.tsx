import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { AddMediaDialog } from "./AddMediaDialog";

const NEW_ID = "7b3f1340-6a2f-4b4a-9c9a-111111111111";

function renderDialog() {
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <AddMediaDialog trigger={<button>Add title</button>} />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("AddMediaDialog", () => {
  it("submits a minimal entry through media_create", async () => {
    vi.mocked(invoke).mockResolvedValue(NEW_ID);
    renderDialog();

    await userEvent.click(screen.getByRole("button", { name: "Add title" }));
    await userEvent.type(await screen.findByLabelText("Title"), "Steins;Gate");
    await userEvent.selectOptions(screen.getByLabelText("Type"), "anime");
    await userEvent.click(screen.getByRole("button", { name: "Add to library" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "media_create",
        expect.objectContaining({ title: "Steins;Gate", contentType: "anime" }),
      );
    });
    expect(await screen.findByText("Title added")).toBeInTheDocument();
  });

  it("shows a validation error for a blank title", async () => {
    renderDialog();

    await userEvent.click(screen.getByRole("button", { name: "Add title" }));
    const title = await screen.findByLabelText("Title");
    fireEvent.focus(title);
    fireEvent.blur(title);
    expect(await screen.findByText("This field is required.")).toBeInTheDocument();
  });

  it("passes genres as a list and empty optionals as null", async () => {
    vi.mocked(invoke).mockResolvedValue(NEW_ID);
    renderDialog();

    await userEvent.click(screen.getByRole("button", { name: "Add title" }));
    await userEvent.type(await screen.findByLabelText("Title"), "Vinland Saga");
    await userEvent.type(screen.getByLabelText("Genres"), "historical, action");
    await userEvent.type(screen.getByLabelText("Release year"), "2005");
    await userEvent.click(screen.getByRole("button", { name: "Add to library" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "media_create",
        expect.objectContaining({
          genres: ["historical", "action"],
          releaseYear: 2005,
          pubStatus: null,
          format: null,
        }),
      );
    });
  });

  it("asks only for the counters the chosen type uses (MISSION-159)", async () => {
    renderDialog();
    await userEvent.click(screen.getByRole("button", { name: "Add title" }));
    const type = await screen.findByLabelText("Type");

    // A film has a runtime — not an episode count, a chapter count or pages.
    await userEvent.selectOptions(type, "movie");
    expect(screen.getByLabelText("Runtime (min)")).toBeInTheDocument();
    expect(screen.queryByLabelText("Episodes")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Chapters")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Pages")).not.toBeInTheDocument();

    // A series has episodes and an episode length, never pages or chapters.
    await userEvent.selectOptions(type, "anime");
    expect(screen.getByLabelText("Episodes")).toBeInTheDocument();
    expect(screen.getByLabelText("Episode length (min)")).toBeInTheDocument();
    expect(screen.queryByLabelText("Pages")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Chapters")).not.toBeInTheDocument();

    // A book counts pages only.
    await userEvent.selectOptions(type, "book");
    expect(screen.getByLabelText("Pages")).toBeInTheDocument();
    expect(screen.queryByLabelText("Chapters")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Episode length (min)")).not.toBeInTheDocument();
  });

  it("drops a counter the new type cannot use", async () => {
    vi.mocked(invoke).mockResolvedValue(NEW_ID);
    renderDialog();

    await userEvent.click(screen.getByRole("button", { name: "Add title" }));
    const type = await screen.findByLabelText("Type");

    await userEvent.selectOptions(type, "book");
    await userEvent.type(screen.getByLabelText("Pages"), "300");
    // A page count left over from the previous type must not be submitted.
    await userEvent.selectOptions(type, "anime");

    await userEvent.type(await screen.findByLabelText("Title"), "Dune");
    await userEvent.click(screen.getByRole("button", { name: "Add to library" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "media_create",
        expect.objectContaining({ pages: null, chCount: null }),
      );
    });
  });
});
