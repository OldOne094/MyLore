import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { api, queryKeys, useGreetMutation, useGreetQuery } from "@/api";
import { createQueryClient } from "@/api/queryClient";

function makeWrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return {
    client,
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  };
}

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
    expect(queryKeys.media.list({ status: "watching" })).toEqual([
      "media",
      "list",
      { status: "watching" },
    ]);
    expect(queryKeys.media.detail(7)).toEqual(["media", "detail", 7]);
    expect(queryKeys.system.greeting("Ada")).toEqual(["system", "greet", "Ada"]);
  });
});

describe("typed command hooks", () => {
  it("api greets through the invoke boundary", async () => {
    vi.mocked(invoke).mockResolvedValue("Hello, Ada! You've been greeted from Rust!");
    await expect(api.greet({ name: "Ada" })).resolves.toContain("Hello, Ada!");
    expect(invoke).toHaveBeenCalledWith("greet", { name: "Ada" });
  });

  it("useGreetQuery resolves and caches under the query key", async () => {
    vi.mocked(invoke).mockResolvedValue("Hello, Ada! You've been greeted from Rust!");
    const { client, wrapper } = makeWrapper();

    const { result } = renderHook(() => useGreetQuery("Ada"), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toContain("Hello, Ada!");
    expect(client.getQueryData(queryKeys.system.greeting("Ada"))).toContain("Hello, Ada!");
    expect(invoke).toHaveBeenCalledWith("greet", { name: "Ada" });
  });

  it("useGreetMutation writes its result into the cache", async () => {
    vi.mocked(invoke).mockResolvedValue("Hello, Grace! You've been greeted from Rust!");
    const { client, wrapper } = makeWrapper();

    const { result } = renderHook(() => useGreetMutation(), { wrapper });
    result.current.mutate({ name: "Grace" });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(client.getQueryData(queryKeys.system.greeting("Grace"))).toContain("Hello, Grace!");
  });
});
