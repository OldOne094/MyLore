import { expect, test, type Page } from "@playwright/test";
import { makeStub, type StubFixtures } from "./ipc-stub";

/* MISSION-097 — End-to-end user flows over the real renderer with the IPC
   boundary stubbed: add media, search, track progress (quick capture),
   import, and backup/restore. Navigation is click-driven (like a real user)
   because the hash router's deep-link on first paint is less representative;
   every flow asserts the correct commands reach the IPC boundary. */

const ROW = {
  id: "m-1",
  content_type: "anime",
  title: "Steins;Gate",
  pub_status: "completed",
  release_year: 2011,
  cover_asset_id: null,
  updated_at: "2026-01-01T00:00:00Z",
  favorite: false,
  progress: { percent: null, completed: 0, total: 0, next_label: null, next_node_id: null },
};

const BASE_FIXTURES: StubFixtures = {
  app_health: { database_ok: true },
  dashboard_summary: { continue_watching: [], recently_completed: [], recently_added: [] },
  media_list: [ROW],
  media_search: [ROW],
  media_facets: { formats: [], genres: [], tags: [], years: [] },
  media_create: "m-new",
  media_nodes: [
    {
      id: "n-1",
      media_id: "m-1",
      kind: "chapter",
      state: "planned",
      number: "1",
      position: 1,
      parent_id: null,
      title: null,
      page_count: null,
      duration_min: null,
      released_at: null,
      created_at: "2026-01-01T00:00:00Z",
      children: [],
    },
  ],
  node_progress_next: null,
  import_file_detect: "anilist",
  import_file_preview: { items: [], total: 0, failed: 0, skipped: 0 },
  trash_list: [],
  collection_list: [],
  providers_list: [],
  review_get: null,
  tracking_get: null,
  backup_prefs_get: { auto_enabled: false, interval_hours: 24, keep_count: 10 },
  backup_restore: {
    id: "t-e2e-restore",
    kind: "restore",
    title: "Restore library backup",
    state: "success",
    progress: 100,
    message: "Restore finished",
    error: null,
    result: {
      media_count: 1,
      asset_count: 0,
      quarantined_to: "C:\\data\\quarantine-x",
      restart_required: true,
    },
    created_at: "2026-08-21T12:05:00Z",
    updated_at: "2026-08-21T12:05:00Z",
  },
  task_get: {
    id: "t-e2e",
    kind: "backup",
    title: "Create library backup",
    state: "success",
    progress: 100,
    message: "Backup finished",
    error: null,
    result: {
      path: "C:\\data\\backups\\mylore-20260821-120000-aaaaaa.mylore",
      size_bytes: 4096,
      media_count: 1,
      asset_count: 0,
    },
    created_at: "2026-08-21T12:00:00Z",
    updated_at: "2026-08-21T12:00:00Z",
  },
  backup_list: [
    {
      file_name: "mylore-20260821-120000-aaaaaa.mylore",
      path: "C:\\data\\backups\\mylore-20260821-120000-aaaaaa.mylore",
      size_bytes: 4096,
      created_at: "20260821120000",
    },
  ],
};

async function gotoDashboard(page: Page, stub: ReturnType<typeof makeStub>) {
  await stub.inject(page);
  await page.goto("/");
  await page.getByRole("heading", { name: "Quick actions" }).waitFor({ timeout: 30_000 });
}

/** Click a nav-rail link and wait for the destination page's h1. */
async function navigateTo(page: Page, linkName: string, headingName: string) {
  await page.getByRole("link", { name: linkName }).click();
  await page.getByRole("heading", { name: headingName, level: 1 }).waitFor({ timeout: 30_000 });
}

test.describe("E2E flows (MISSION-097)", () => {
  test("adds a title through the dashboard quick action", async ({ page }) => {
    const stub = makeStub(BASE_FIXTURES);
    await gotoDashboard(page, stub);

    await page.getByRole("button", { name: "Add title" }).click();
    await page.getByRole("heading", { name: "Add a title" }).waitFor();
    await page.getByLabel("Title", { exact: true }).fill("Dune");
    await page.getByRole("button", { name: "Add to library" }).click();

    await expect
      .poll(() => stub.calls(page, "media_create"))
      .toMatchObject([{ args: expect.objectContaining({ title: "Dune" }) }]);
    await page.getByText("Title added").first().waitFor();
  });

  test("searches the library and renders result links", async ({ page }) => {
    const stub = makeStub(BASE_FIXTURES);
    await gotoDashboard(page, stub);

    const searchbox = page.getByRole("searchbox", { name: "Search your library" });
    await searchbox.fill("steins");
    await searchbox.press("Enter");

    await page.waitForURL(/q=steins/);
    const link = page.getByRole("link", { name: "Steins;Gate" });
    await link.waitFor();
    await expect(link).toHaveAttribute("href", "#/library/m-1");
    await expect.poll(() => stub.calls(page, "media_search")).not.toHaveLength(0);
  });

  test("tracks progress through quick capture", async ({ page }) => {
    const stub = makeStub(BASE_FIXTURES);
    await gotoDashboard(page, stub);

    await page.getByRole("button", { name: "Quick capture" }).click();
    const dialog = page.getByRole("dialog");
    const input = dialog.getByRole("combobox", { name: /Search your library/ });
    await input.waitFor();
    await input.fill("steins");

    // Type-ahead (debounced) then pick the title.
    await dialog.getByRole("option", { name: /Steins;Gate/ }).click();
    await dialog.getByRole("button", { name: "Mark next done" }).click();

    await expect.poll(() => stub.calls(page, "node_progress_next")).toHaveLength(1);
  });

  test("imports a file up to the preview stage", async ({ page }) => {
    const stub = makeStub(BASE_FIXTURES);
    await gotoDashboard(page, stub);
    await navigateTo(page, "Library", "Library");

    await page.getByRole("button", { name: "Import" }).click();
    await page.setInputFiles('input[type="file"]', {
      name: "anilist_export.json",
      mimeType: "application/json",
      buffer: Buffer.from(JSON.stringify({ entries: [] })),
    });

    await expect.poll(() => stub.calls(page, "import_file_detect")).not.toHaveLength(0);
    const detect = (await stub.calls(page, "import_file_detect"))[0];
    expect((detect?.args as { source?: string } | undefined)?.source).toContain("{");
    await expect.poll(() => stub.calls(page, "import_file_preview")).not.toHaveLength(0);
  });

  test("creates a backup, lists it, and restores it", async ({ page }) => {
    const stub = makeStub(BASE_FIXTURES);
    await gotoDashboard(page, stub);
    await navigateTo(page, "Settings", "Settings");

    // Create: spawns the background task.
    await page.getByRole("button", { name: "Back up now" }).click();
    await expect.poll(() => stub.calls(page, "backup_create")).toHaveLength(1);

    // List: the archive row renders (exact match — the success toast embeds
    // the same file name).
    await page
      .getByText("mylore-20260821-120000-aaaaaa.mylore", { exact: true })
      .waitFor({ timeout: 15_000 });

    // Restore through the guarded dialog.
    await page.getByRole("button", { name: "Restore" }).first().click();
    const dialog = page.getByRole("dialog");
    await dialog.getByText(/replaced by this archive/i).waitFor();
    await dialog.getByRole("button", { name: "Restore" }).last().click();
    await expect.poll(() => stub.calls(page, "backup_restore")).toHaveLength(1);
    await page.getByText(/restart MyLore/i).waitFor();
  });
});
