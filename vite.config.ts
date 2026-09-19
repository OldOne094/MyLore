import { defineConfig, configDefaults } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";
import { version as appVersion } from "./package.json";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  // The status bar shows the version the build was cut from. `package.json` is
  // the source `scripts/release.mjs` keeps in sync with `Cargo.toml` and
  // `tauri.conf.json`, so the UI never carries a hand-edited second copy.
  define: { __APP_VERSION__: JSON.stringify(appVersion) },

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    // Playwright specs live in e2e/ and are run by `npm run e2e`, not Vitest.
    // `.freebuff/**` is a local worktree scratch dir (a full stale copy of the
    // repo) that must not double-run the suites or fail on older code.
    exclude: [...configDefaults.exclude, "**/e2e/**", "**/.freebuff/**"],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
