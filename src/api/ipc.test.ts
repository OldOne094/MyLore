import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { greet } from "@/api";

describe("typed IPC wrappers", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("greet invokes the command with typed args and resolves the result", async () => {
    vi.mocked(invoke).mockResolvedValue("Hello, A! You've been greeted from Rust!");

    const result = await greet({ name: "A" });

    expect(result).toContain("Hello, A!");
    expect(invoke).toHaveBeenCalledWith("greet", { name: "A" });
  });
});
