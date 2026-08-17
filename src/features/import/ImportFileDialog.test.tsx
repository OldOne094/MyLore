import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Button, ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { ImportFileDialog } from "./ImportFileDialog";
import type { ImportPreview, ImportReport } from "@/api";

let fakeFileText = "";

class FakeFileReader {
  onload: (() => void) | null = null;
  result = "";
  readAsText() {
    this.result = fakeFileText;
    this.onload?.();
  }
}

const PREVIEW: ImportPreview = {
  total: 4,
  valid: 2,
  invalid: 1,
  new: 2,
  in_library: 1,
  duplicates: 1,
  items: [
    {
      source_row: 1,
      title: "Sword",
      outcome: "new",
      matched_media_id: null,
      match_kind: null,
      match_score: null,
      issues: [],
    },
    {
      source_row: 2,
      title: "Berserk",
      outcome: "new",
      matched_media_id: null,
      match_kind: null,
      match_score: null,
      issues: [],
    },
    {
      source_row: 3,
      title: "Sword of the Dawn",
      outcome: "duplicate",
      matched_media_id: "m-1",
      match_kind: "duplicate",
      match_score: 0.95,
      issues: [],
    },
    {
      source_row: 4,
      title: null,
      outcome: "invalid",
      matched_media_id: null,
      match_kind: null,
      match_score: null,
      issues: [{ severity: "error", field: "title", message: "blank title" }],
    },
  ],
};

const REPORT: ImportReport = {
  total: 2,
  committed: 2,
  skipped: 0,
  failed: 0,
  items: [
    { source_row: 1, title: "Sword", status: "committed", media_id: "m-1", message: null },
    { source_row: 2, title: "Berserk", status: "committed", media_id: "m-2", message: null },
  ],
};

function renderDialog() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <ImportFileDialog trigger={<Button>Open import</Button>} />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  fakeFileText = "";
  vi.stubGlobal("FileReader", FakeFileReader);
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "import_csv_headers":
        return ["Title", "Author", "Genres"];
      case "import_file_preview":
        return PREVIEW;
      case "import_commit":
        return REPORT;
      default:
        throw new Error(`unexpected command ${cmd}`);
    }
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  vi.unstubAllGlobals();
  await i18n.changeLanguage("en");
});

describe("ImportFileDialog", () => {
  it("previews a JSON file and imports the selected new rows", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '[{"title":"Sword","content_type":"novel"},{"title":"Berserk"}]';
    const input = screen.getByLabelText("Choose file");
    await user.upload(input, new File([fakeFileText], "books.json", { type: "application/json" }));

    expect(await screen.findByText("Review titles before importing")).toBeInTheDocument();
    expect(screen.getByText("2 new")).toBeInTheDocument();
    expect(screen.getByText("1 in library")).toBeInTheDocument();
    expect(screen.getByText("1 duplicates")).toBeInTheDocument();
    expect(screen.getByText("1 invalid")).toBeInTheDocument();

    expect(invoke).toHaveBeenCalledWith("import_file_preview", {
      kind: "json",
      source: fakeFileText,
      mapping: null,
    });

    await user.click(screen.getByRole("button", { name: "Import 2 titles" }));

    expect(invoke).toHaveBeenCalledWith("import_commit", {
      kind: "json",
      source: fakeFileText,
      mapping: null,
      plan: { rows: [1, 2] },
    });
    expect(await screen.findByText("Import finished")).toBeInTheDocument();
    expect(screen.getByText("2 titles added · 0 skipped · 0 failed")).toBeInTheDocument();
  });

  it("maps CSV columns, previews, and sends the mapping + selected rows", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = 'Title,Author,Genres\nSword,Jane,"Fantasy, Adventure"\nBerserk,Miura,Seinen';
    const input = screen.getByLabelText("Choose file");
    await user.upload(input, new File([fakeFileText], "books.csv", { type: "text/csv" }));

    await waitFor(() => {
      expect(screen.getByLabelText("Title")).toBeInTheDocument();
    });
    expect(invoke).toHaveBeenCalledWith("import_csv_headers", {
      source: fakeFileText,
      delimiter: ",",
    });

    expect(screen.getByRole("button", { name: "Import 0 titles" })).toBeDisabled();
    await user.selectOptions(screen.getByLabelText("Title"), "Title");
    await user.selectOptions(screen.getByLabelText("Author"), "Author");
    await user.selectOptions(screen.getByLabelText("Genres"), "Genres");

    expect(await screen.findByText("Review titles before importing")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Import 2 titles" }));

    expect(invoke).toHaveBeenCalledWith("import_commit", {
      kind: "csv",
      source: fakeFileText,
      mapping: expect.objectContaining({
        title: "Title",
        author: "Author",
        genres: "Genres",
        delimiter: ",",
      }),
      plan: { rows: [1, 2] },
    });
    expect(await screen.findByText("Import finished")).toBeInTheDocument();
  });

  it("shows per-item outcomes, keeps invalid rows out of the plan, and lets the user deselect", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = "Title\nSword\nBerserk\nSword of the Dawn\n\n";
    await user.upload(
      screen.getByLabelText("Choose file"),
      new File([fakeFileText], "books.csv", { type: "text/csv" }),
    );

    await user.selectOptions(await screen.findByLabelText("Title"), "Title");
    await screen.findByText("Review titles before importing");

    expect(screen.getAllByText("New")).toHaveLength(2);
    expect(screen.getAllByText("Duplicate")).toHaveLength(1);
    expect(screen.getAllByText("Invalid")).toHaveLength(1);
    expect(screen.getByText(/blank title/)).toBeInTheDocument();

    const row4 = screen.getByLabelText("Select row 4");
    expect(row4).toBeDisabled();

    expect(screen.getByLabelText("Select all new titles")).toBeChecked();
    await user.click(screen.getByLabelText("Select row 1"));
    expect(screen.getByLabelText("Select all new titles")).not.toBeChecked();

    await user.click(screen.getByRole("button", { name: "Import 1 title" }));
    expect(invoke).toHaveBeenCalledWith(
      "import_commit",
      expect.objectContaining({ plan: { rows: [2] } }),
    );
  });

  it("disables import when nothing new remains after deselecting every row", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '[{"title":"Sword"},{"title":"Berserk"}]';
    await user.upload(
      screen.getByLabelText("Choose file"),
      new File([fakeFileText], "books.json", { type: "application/json" }),
    );

    await screen.findByText("Review titles before importing");
    await user.click(screen.getByLabelText("Select all new titles"));
    expect(screen.getByRole("button", { name: "Import 0 titles" })).toBeDisabled();
  });

  it("surfaces an import failure as a toast", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "import_commit") throw new Error("import error: boom");
      if (cmd === "import_csv_headers") return ["Title"];
      if (cmd === "import_file_preview") return PREVIEW;
      throw new Error(`unexpected command ${cmd}`);
    });
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = "Title\nSword";
    await user.upload(
      screen.getByLabelText("Choose file"),
      new File([fakeFileText], "books.json", { type: "application/json" }),
    );

    await user.click(await screen.findByRole("button", { name: "Import 2 titles" }));
    expect(await screen.findByText("Couldn't import the file")).toBeInTheDocument();
  });
});
