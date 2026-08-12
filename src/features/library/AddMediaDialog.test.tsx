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
        expect.objectContaining({ title: "Steins;Gate", content_type: "anime" }),
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
          release_year: 2005,
          pub_status: null,
          format: null,
        }),
      );
    });
  });
});
