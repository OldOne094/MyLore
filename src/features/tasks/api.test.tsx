/* MISSION-118 hotfix — the missed-event path. A short task can reach its
   terminal state *before* the `task-changed` subscription exists (the spawn
   resolves first, and `listen` is awaited asynchronously). These tests mock a
   listener that never delivers, which is how the UI used to get stuck: the
   fallback read updated the snapshot but never ran `onSuccess`, and a running
   snapshot was never read again. */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useTask } from "./api";

const task_get = vi.fn();
const listenTaskChanged = vi.fn();

vi.mock("@/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/api")>();
  return {
    ...actual,
    task_get: (...args: unknown[]) => task_get(...args),
    listenTaskChanged: (...args: unknown[]) => listenTaskChanged(...args),
    task_cancel: vi.fn(),
  };
});

function snapshot(state: string, overrides: Record<string, unknown> = {}) {
  return {
    id: "t-1",
    kind: "import_file",
    title: "Import",
    state,
    progress: null,
    message: null,
    error: null,
    result: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function wrapper(queryClient: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe("useTask", () => {
  beforeEach(() => {
    task_get.mockReset();
    listenTaskChanged.mockReset();
    // A subscription that is registered but never delivers — the race in full.
    listenTaskChanged.mockResolvedValue(() => {});
  });

  it("still reports success when the event never arrives", async () => {
    task_get.mockResolvedValue(snapshot("success", { result: { imported: 3 } }));
    const onSuccess = vi.fn();

    const { result } = renderHook(() => useTask("t-1", { onSuccess }), {
      wrapper: wrapper(new QueryClient()),
    });

    await waitFor(() => expect(result.current.data?.state).toBe("success"));
    await waitFor(() => expect(onSuccess).toHaveBeenCalledTimes(1));
  });

  it("polls a running task until it ends", async () => {
    task_get
      .mockResolvedValueOnce(snapshot("running", { progress: 10 }))
      .mockResolvedValue(snapshot("success", { progress: 100 }));
    const onSuccess = vi.fn();

    const { result } = renderHook(() => useTask("t-1", { onSuccess }), {
      wrapper: wrapper(new QueryClient()),
    });

    await waitFor(() => expect(result.current.data?.state).toBe("success"), { timeout: 4000 });
    expect(task_get.mock.calls.length).toBeGreaterThan(1);
  });

  it("reports success once, not on every re-render", async () => {
    task_get.mockResolvedValue(snapshot("success"));
    const onSuccess = vi.fn();

    const { result, rerender } = renderHook(() => useTask("t-1", { onSuccess }), {
      wrapper: wrapper(new QueryClient()),
    });

    await waitFor(() => expect(onSuccess).toHaveBeenCalledTimes(1));
    rerender();
    await waitFor(() => expect(result.current.data?.state).toBe("success"));
    expect(onSuccess).toHaveBeenCalledTimes(1);
  });

  it("does not fetch without a task id", () => {
    renderHook(() => useTask(null), { wrapper: wrapper(new QueryClient()) });
    expect(task_get).not.toHaveBeenCalled();
  });
});
