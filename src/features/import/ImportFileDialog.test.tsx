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
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
  emit: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ImportFileDialog } from "./ImportFileDialog";
import type { ImportPreview, ImportReport, TaskSnapshot } from "@/api";

let fakeFileText = "";

/** Mirrors the backend `import_file_detect` sniffing for the test fixtures. */
function detectKind(text: string): string {
  const trimmed = text.trimStart().replace(/^\uFEFF/, "");
  if (trimmed.startsWith("[")) return "json";
  if (trimmed.startsWith("{")) {
    return trimmed.includes("mediaListCollection") ? "anilist" : "json";
  }
  if (/"?Book Id"?/i.test(trimmed)) return "goodreads";
  if (/Reading Status/i.test(trimmed)) return "storygraph";
  return "csv";
}

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

const TASK_ID = "t-1";
let currentTask: TaskSnapshot;
let taskListener: ((snapshot: TaskSnapshot) => void) | undefined;

function makeSnapshot(
  state: TaskSnapshot["state"],
  overrides: Partial<TaskSnapshot> = {},
): TaskSnapshot {
  return {
    id: TASK_ID,
    kind: "import_file",
    title: "Import json file",
    state,
    progress: null,
    message: null,
    error: null,
    result: null,
    created_at: "2026-08-16T00:00:00Z",
    updated_at: "2026-08-16T00:00:00Z",
    ...overrides,
  };
}

function emitTask(next: Partial<TaskSnapshot>) {
  currentTask = { ...currentTask, ...next, id: TASK_ID };
  taskListener?.(currentTask);
}

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
  taskListener = undefined;
  currentTask = makeSnapshot("running", { progress: 0, message: "Analyzing the file…" });
  vi.stubGlobal("FileReader", FakeFileReader);
  vi.mocked(listen).mockImplementation(((
    _event: string,
    handler: (event: { payload: TaskSnapshot }) => void,
  ) => {
    taskListener = (snapshot) => handler({ payload: snapshot });
    return Promise.resolve(() => undefined);
  }) as typeof listen);
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "import_file_detect":
        return detectKind(fakeFileText);
      case "import_csv_headers":
        return ["Title", "Author", "Genres"];
      case "import_file_preview":
        return PREVIEW;
      case "import_commit":
        return currentTask;
      case "task_get":
        return currentTask;
      default:
        throw new Error(`unexpected command ${cmd}`);
    }
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
  vi.unstubAllGlobals();
  taskListener = undefined;
  await i18n.changeLanguage("en");
});

describe("ImportFileDialog", () => {
  it("previews a JSON file and imports the selected new rows as a task", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '[{"title":"Sword","content_type":"novel"},{"title":"Berserk"}]';
    const input = screen.getByLabelText("Choose file");
    await user.upload(input, new File([fakeFileText], "books.json", { type: "application/json" }));

    expect(await screen.findByText("Review titles before importing")).toBeInTheDocument();
    expect(screen.getByText("2 new")).toBeInTheDocument();

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

    expect(await screen.findByRole("status")).toBeInTheDocument();
    expect(screen.getByText("Importing…")).toBeInTheDocument();

    emitTask({ state: "running", progress: 50, message: "Importing 1/2 titles" });
    expect(await screen.findByText("Importing 1/2 titles")).toBeInTheDocument();

    emitTask({ state: "success", progress: 100, result: REPORT });
    expect(await screen.findByText("Import finished")).toBeInTheDocument();
    expect(screen.getByText(/2 titles added/)).toBeInTheDocument();
  }, 20_000);

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

    emitTask({ state: "success", progress: 100, result: REPORT });
    expect(await screen.findByText("Import finished")).toBeInTheDocument();
  }, 20_000);

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

  it("surfaces an immediate import failure as a toast", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "import_file_detect") return detectKind(fakeFileText);
      if (cmd === "import_commit") throw new Error("import error: boom");
      if (cmd === "import_csv_headers") return ["Title"];
      if (cmd === "import_file_preview") return PREVIEW;
      throw new Error(`unexpected command ${cmd}`);
    });
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '[{"title":"Sword"}]';
    await user.upload(
      screen.getByLabelText("Choose file"),
      new File([fakeFileText], "books.json", { type: "application/json" }),
    );

    await user.click(await screen.findByRole("button", { name: "Import 2 titles" }));
    expect(await screen.findByText("Couldn't import the file")).toBeInTheDocument();
  });

  it("cancels a running import and shows the cancelled state", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '[{"title":"Sword"}]';
    await user.upload(
      screen.getByLabelText("Choose file"),
      new File([fakeFileText], "books.json", { type: "application/json" }),
    );

    await user.click(await screen.findByRole("button", { name: "Import 2 titles" }));
    await screen.findByRole("status");

    await user.click(screen.getByRole("button", { name: "Cancel import" }));
    expect(invoke).toHaveBeenCalledWith("task_cancel", { id: TASK_ID });

    emitTask({ state: "cancelled" });
    expect(await screen.findByText("Import cancelled")).toBeInTheDocument();
  });

  it("shows a task failure with the backend message", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '[{"title":"Sword"}]';
    await user.upload(
      screen.getByLabelText("Choose file"),
      new File([fakeFileText], "books.json", { type: "application/json" }),
    );

    await user.click(await screen.findByRole("button", { name: "Import 2 titles" }));
    await screen.findByRole("status");

    emitTask({ state: "failed", error: "database error: locked" });
    expect(await screen.findByText("Couldn't import the file")).toBeInTheDocument();
    expect(screen.getByText("database error: locked")).toBeInTheDocument();
  });

  it("detects a Goodreads export, shows the profile badge, and skips the mapping table", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '"Book Id","Title","Author","My Rating","Exclusive Shelf"\n1,Sword,Jane,4,read';
    const input = screen.getByLabelText("Choose file");
    await user.upload(input, new File([fakeFileText], "library_export.csv", { type: "text/csv" }));

    expect(await screen.findByText("Goodreads export")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Profile export — your status, rating, and progress will be imported automatically. No mapping needed.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByLabelText("CSV delimiter")).not.toBeInTheDocument();

    expect(await screen.findByText("Review titles before importing")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("import_file_preview", {
      kind: "goodreads",
      source: fakeFileText,
      mapping: null,
    });

    await user.click(screen.getByRole("button", { name: "Import 2 titles" }));
    expect(invoke).toHaveBeenCalledWith(
      "import_commit",
      expect.objectContaining({ kind: "goodreads", mapping: null }),
    );
  });

  it("detects an AniList export as a profile import", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("button", { name: "Open import" }));

    fakeFileText = '{"mediaListCollection":{"lists":[{"entries":[{"media":{"id":1}}]}]}}';
    await user.upload(
      screen.getByLabelText("Choose file"),
      new File([fakeFileText], "anilist.json", { type: "application/json" }),
    );

    expect(await screen.findByText("AniList export")).toBeInTheDocument();
    expect(await screen.findByText("Review titles before importing")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("import_file_preview", {
      kind: "anilist",
      source: fakeFileText,
      mapping: null,
    });
  });
});
