// Guards the Tauri security/window config (MISSION-010): the production CSP
// must stay strict, and the window shell must enforce minimum sizes.

import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";

const config = JSON.parse(
  readFileSync(path.join(process.cwd(), "src-tauri", "tauri.conf.json"), "utf8"),
);

describe("Tauri app config", () => {
  it("uses the MyLore product identity", () => {
    expect(config.productName).toBe("MyLore");
    expect(config.identifier).toBe("com.mylore.app");
  });

  it("enforces a minimum window size", () => {
    const [window] = config.app.windows;
    expect(window.label).toBe("main");
    expect(window.minWidth).toBeGreaterThanOrEqual(900);
    expect(window.minHeight).toBeGreaterThanOrEqual(600);
  });

  it("sets a production CSP without unsafe-inline scripts", () => {
    const csp = config.app.security.csp;
    expect(csp).toMatch(/default-src 'self'/);
    expect(csp).toMatch(/object-src 'none'/);
    expect(csp).not.toMatch(/script-src [^;]*'unsafe-inline'/);
  });
});
