// Guards against drift between scripts/ipc-contract.json and the generated
// src/api/ipc.generated.ts. Runs the real codegen in --check mode.

import { describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import path from "node:path";

const script = path.join(process.cwd(), "scripts", "codegen-ipc.mjs");

describe("IPC codegen", () => {
  it("generated types are in sync with the contract (codegen:check passes)", () => {
    expect(() =>
      execFileSync(process.execPath, [script, "--check"], { encoding: "utf8" }),
    ).not.toThrow();
  });
});
