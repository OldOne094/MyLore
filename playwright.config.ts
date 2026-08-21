import { defineConfig } from "@playwright/test";

/* MISSION-097 — E2E suite. Runs the real renderer (Vite dev server) in an
   installed Edge/Chromium with the Tauri IPC boundary replaced by a scripted
   stub (see e2e/ipc-stub.ts), so user flows are exercised end-to-end and
   deterministically — no WebView2 driver or native window required. */

export default defineConfig({
  testDir: "./e2e",
  timeout: 60_000,
  retries: 0,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:1420",
    channel: "msedge",
    viewport: { width: 1280, height: 800 },
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
