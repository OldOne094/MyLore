import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "@/api";
import { createQueryClient } from "@/api/queryClient";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("query client", () => {
  it("applies local-first query defaults", () => {
    const client = createQueryClient();
    const options = client.getDefaultOptions().queries ?? {};
    expect(options.retry).toBe(false);
    expect(options.staleTime).toBe(60_000);
    expect(options.refetchOnWindowFocus).toBe(false);
    expect(client.getDefaultOptions().mutations?.retry).toBe(false);
  });
});

describe("query keys", () => {
  it("builds namespaced, fan-out friendly keys", () => {
    expect(queryKeys.media.all()).toEqual(["media"]);
    expect(queryKeys.media.list({ pub_status: "watching" })).toEqual([
      "media",
      "list",
      { pub_status: "watching" },
    ]);
    expect(queryKeys.media.detail("m-7")).toEqual(["media", "detail", "m-7"]);
    expect(queryKeys.review.list()).toEqual(["review", "list"]);
  });
});
