/// <reference types="vite/client" />

/** App version, injected from `package.json` at build time (see vite.config.ts).
    Single source with `Cargo.toml`/`tauri.conf.json`, kept in sync by
    `scripts/release.mjs`. */
declare const __APP_VERSION__: string;
