import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { ReviewTab } from "./ReviewTab";
import type { ReviewView } from "@/api";

const REVIEW: ReviewView = {
  media_id: "m-111",
  rating: 8,
  review: "A sweeping epic.",
  short_review: null,
  notes: "Re-read after the anime.",
  favorite: true,
  is_spoiler: true,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-02T00:00:00Z",
};

const TAGS = [
  { id: "tag-1", name: "cozy", scope: "personal" },
  { id: "tag-2", name: "re-read", scope: "personal" },
];

function renderTab() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <ReviewTab mediaId="m-111" />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

function mockReview(view: ReviewView | null, tags = TAGS) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "review_get") return Promise.resolve(view);
    if (cmd === "media_tags") return Promise.resolve(tags);
    if (cmd === "media_add_tag" || cmd === "media_remove_tag") return Promise.resolve(tags);
    if (cmd === "review_save")
      return Promise.resolve({
        media_id: "m-111",
        rating: null,
        review: null,
        short_review: null,
        notes: null,
        favorite: false,
        is_spoiler: false,
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-02T00:00:00Z",
      });
    return Promise.resolve(undefined);
  });
}

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("ReviewTab", () => {
  it("loads and renders an existing review's values and personal tags", async () => {
    mockReview(REVIEW);
    renderTab();

    expect(await screen.findByRole("textbox", { name: "Review" })).toHaveValue("A sweeping epic.");
    expect(screen.getByRole("textbox", { name: "Notes" })).toHaveValue("Re-read after the anime.");
    expect(screen.getByRole("button", { name: "Favorite" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "8" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("checkbox", { name: /Contains spoilers/ })).toBeChecked();
    expect(screen.getByText("cozy")).toBeInTheDocument();
    expect(screen.getByText("re-read")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("review_get", { media_id: "m-111" });
    expect(invoke).toHaveBeenCalledWith("media_tags", { media_id: "m-111" });
  });

  it("shows the empty-state hint when no review exists", async () => {
    mockReview(null, []);
    renderTab();

    expect(await screen.findByText("No review yet — write one below.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Clear review" })).not.toBeInTheDocument();
  });

  it("saves the form values through review_save and shows a success toast", async () => {
    mockReview(null, []);
    renderTab();
    await screen.findByText("No review yet — write one below.");

    const review = screen.getByRole("textbox", { name: "Review" });
    await userEvent.type(review, "Slow burn, worth it.");
    await userEvent.type(screen.getByRole("textbox", { name: "Short review" }), "Slow burn.");
    await userEvent.click(screen.getByRole("button", { name: "Save review" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("review_save", {
        media_id: "m-111",
        rating: null,
        review: "Slow burn, worth it.",
        short_review: "Slow burn.",
        notes: null,
        favorite: false,
        is_spoiler: false,
      }),
    );
    expect(await screen.findByText("Review saved")).toBeInTheDocument();
  });

  it("toggles favorite and clears a rating by clicking it again", async () => {
    mockReview(null, []);
    renderTab();
    await screen.findByText("No review yet — write one below.");

    await userEvent.click(screen.getByRole("button", { name: "Favorite" }));
    await userEvent.click(screen.getByRole("button", { name: "7" }));
    expect(screen.getByRole("button", { name: "7" })).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(screen.getByRole("button", { name: "7" }));
    expect(screen.getByRole("button", { name: "7" })).toHaveAttribute("aria-pressed", "false");

    await userEvent.click(screen.getByRole("button", { name: "Save review" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("review_save", {
        media_id: "m-111",
        rating: null,
        review: null,
        short_review: null,
        notes: null,
        favorite: true,
        is_spoiler: false,
      }),
    );
  });

  it("submits the spoiler flag with the review text", async () => {
    mockReview(null, []);
    renderTab();
    await screen.findByText("No review yet — write one below.");

    await userEvent.type(screen.getByRole("textbox", { name: "Review" }), "The twist is X.");
    await userEvent.click(screen.getByRole("checkbox", { name: /Contains spoilers/ }));
    await userEvent.click(screen.getByRole("button", { name: "Save review" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("review_save", {
        media_id: "m-111",
        rating: null,
        review: "The twist is X.",
        short_review: null,
        notes: null,
        favorite: false,
        is_spoiler: true,
      }),
    );
  });

  it("clears the review through review_delete", async () => {
    mockReview(REVIEW);
    renderTab();
    expect(await screen.findByRole("textbox", { name: "Review" })).toHaveValue("A sweeping epic.");

    await userEvent.click(screen.getByRole("button", { name: "Clear review" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("review_delete", { media_id: "m-111" }),
    );
    expect(await screen.findByText("No review yet — write one below.")).toBeInTheDocument();
  });

  it("adds and removes personal tags through the tag commands", async () => {
    mockReview(null, TAGS);
    renderTab();
    await screen.findByText("No review yet — write one below.");

    await userEvent.type(screen.getByLabelText("Add a tag…"), "shelf");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("media_add_tag", { media_id: "m-111", tag: "shelf" }),
    );
    expect(await screen.findByText("Tag added")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /Tag removed: cozy/ }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("media_remove_tag", {
        media_id: "m-111",
        tag_id: "tag-1",
      }),
    );
  });

  it("shows an error toast when the save fails", async () => {
    mockReview(null, []);
    renderTab();
    await screen.findByText("No review yet — write one below.");

    vi.mocked(invoke).mockRejectedValueOnce("boom");
    await userEvent.click(screen.getByRole("button", { name: "Save review" }));

    expect(await screen.findByText("Couldn't save the review")).toBeInTheDocument();
  });
});
