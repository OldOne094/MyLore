// Guards against drift between scripts/ipc-contract.json and the generated
// src/api/ipc.generated.ts. Runs the real codegen in --check mode.

import { describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import path from "node:path";

const script = path.join(process.cwd(), "scripts", "codegen-ipc.mjs");

describe("IPC codegen", () => {
  // The check spawns node + prettier's format loop, which legitimately takes
  // several seconds — and much more under the full suite's parallel load.
  // The default 5s testTimeout flakes; this subprocess deserves real headroom.
  it(
    "generated types are in sync with the contract (codegen:check passes)",
    { timeout: 60_000 },
    () => {
      expect(() =>
        execFileSync(process.execPath, [script, "--check"], { encoding: "utf8" }),
      ).not.toThrow();
    },
  );
});
