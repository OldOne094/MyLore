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
import type { ImportReport } from "@/api";

let fakeFileText = "";

class FakeFileReader {
  onload: (() => void) | null = null;
  result = "";
  readAsText() {
    this.result = fakeFileText;
    this.onload?.();
  }
}

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
  it("imports a JSON file directly", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '[{"title":"Sword","content_type":"novel"}]';
    const input = screen.getByLabelText("Choose file");
    await user.upload(input, new File([fakeFileText], "books.json", { type: "application/json" }));

    expect(
      await screen.findByText("JSON file ready — importing will add every new title."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Import" }));

    expect(invoke).toHaveBeenCalledWith("import_commit", {
      kind: "json",
      source: '[{"title":"Sword","content_type":"novel"}]',
      mapping: null,
      plan: null,
    });
    expect(await screen.findByText("Import finished")).toBeInTheDocument();
    expect(screen.getByText("2 titles added · 0 skipped · 0 failed")).toBeInTheDocument();
  });

  it("maps CSV columns before importing and sends the mapping", async () => {
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

    expect(screen.getByRole("button", { name: "Import" })).toBeDisabled();
    await user.selectOptions(screen.getByLabelText("Title"), "Title");
    await user.selectOptions(screen.getByLabelText("Author"), "Author");
    await user.selectOptions(screen.getByLabelText("Genres"), "Genres");

    await user.click(screen.getByRole("button", { name: "Import" }));
    expect(invoke).toHaveBeenCalledWith("import_commit", {
      kind: "csv",
      source: fakeFileText,
      mapping: expect.objectContaining({
        title: "Title",
        author: "Author",
        genres: "Genres",
        delimiter: ",",
      }),
      plan: null,
    });
    expect(await screen.findByText("Import finished")).toBeInTheDocument();
  });

  it("keeps the import disabled until a CSV title column is mapped", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = "Title,Author\nSword,Jane";
    await user.upload(
      screen.getByLabelText("Choose file"),
      new File([fakeFileText], "books.csv", { type: "text/csv" }),
    );

    expect(await screen.findByText("Map the Title column to import.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Import" })).toBeDisabled();

    await user.selectOptions(screen.getByLabelText("Title"), "Title");
    expect(screen.getByRole("button", { name: "Import" })).toBeEnabled();
  });

  it("surfaces an import failure as a toast", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "import_commit") throw new Error("import error: boom");
      if (cmd === "import_csv_headers") return ["Title"];
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

    await user.click(await screen.findByRole("button", { name: "Import" }));
    expect(await screen.findByText("Couldn't import the file")).toBeInTheDocument();
  });
});
