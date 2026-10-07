import type { Page } from "@playwright/test";

/* MISSION-097 — Tauri IPC stub for E2E. Injected as an init script before any
   app code runs: replaces `window.__TAURI_INTERNALS__` (the boundary
   `@tauri-apps/api/core` delegates to) with a scripted backend that serves
   fixture responses, records every invocation for assertions, and fakes the
   plugin surfaces the flows touch (event listen, store-backed preferences,
   file dialogs). */

export type StubFixtures = Record<string, unknown>;

export interface IpcStub {
  inject: (page: Page) => Promise<void>;
  /** Every recorded invocation of `command`, in order. */
  calls: (page: Page, command: string) => Promise<{ command: string; args: unknown }[]>;
  setDialogPath: (page: Page, path: string | null) => Promise<void>;
}

export function makeStub(fixtures: StubFixtures): IpcStub {
  const script = `
    (() => {
      const fixtures = ${JSON.stringify(fixtures)};
      const calls = [];
      window.__ipcCalls = calls;
      window.__dialogOpenPath = null;

      const stores = new Map();
      let nextId = 1;

      // The real store is a file on disk. A plain Map would be rebuilt by every
      // reload (this init script re-runs), so back it with localStorage: a
      // reload then behaves like a relaunch, which is the only way an E2E test
      // can observe that a setting was persisted at all (MISSION-158).
      const STORE_KEY = "__myloreStore";
      function snapshot() {
        try {
          return JSON.parse(localStorage.getItem(STORE_KEY) || "{}");
        } catch {
          return {};
        }
      }
      function commit(data) {
        try {
          localStorage.setItem(STORE_KEY, JSON.stringify(data));
        } catch {
          /* Storage unavailable — the store stays in-memory for this page. */
        }
      }

      function handleStore(cmd, args) {
        if (cmd === "plugin:store|load") {
          const rid = nextId++;
          stores.set(rid, { data: snapshot() });
          return Promise.resolve(rid);
        }
        const store = stores.get(args?.rid);
        if (!store) return Promise.resolve(null);
        const key = args?.key;
        const hasKey = () => Object.prototype.hasOwnProperty.call(store.data, key);
        // The plugin answers get with a [value, exists] tuple, and has with a
        // boolean. Returning the bare value here is not destructured: it reads
        // as "absent", so no store-backed setting is ever seen as persisted.
        if (cmd === "plugin:store|get") {
          return Promise.resolve([hasKey() ? store.data[key] : null, hasKey()]);
        }
        if (cmd === "plugin:store|set") {
          store.data[key] = args.value;
          commit(store.data);
          return Promise.resolve(null);
        }
        if (cmd === "plugin:store|has") return Promise.resolve(hasKey());
        if (cmd === "plugin:store|keys") return Promise.resolve(Object.keys(store.data));
        if (cmd === "plugin:store|values") return Promise.resolve(Object.values(store.data));
        if (cmd === "plugin:store|entries") return Promise.resolve(Object.entries(store.data));
        if (cmd === "plugin:store|length") return Promise.resolve(Object.keys(store.data).length);
        if (cmd === "plugin:store|delete") {
          delete store.data[key];
          commit(store.data);
          return Promise.resolve(null);
        }
        if (cmd === "plugin:store|clear") {
          store.data = {};
          commit(store.data);
          return Promise.resolve(null);
        }
        if (cmd === "plugin:store|save" || cmd === "plugin:store|reload") {
          commit(store.data);
          return Promise.resolve(null);
        }
        return Promise.resolve(null);
      }

      window.__TAURI_INTERNALS__ = {
        transformCallback: () => nextId++,
        metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
        plugins: {},
        invoke: (command, args) => {
          calls.push({ command, args });
          if (command === "plugin:event|listen" || command === "plugin:event|unlisten") {
            return Promise.resolve(nextId++);
          }
          if (command?.startsWith("plugin:store|")) return handleStore(command, args);
          if (command === "plugin:dialog|open") {
            return Promise.resolve(window.__dialogOpenPath ?? null);
          }
          if (command === "plugin:dialog|save") return Promise.resolve(null);
          if (Object.prototype.hasOwnProperty.call(fixtures, command)) {
            const value = fixtures[command];
            return Promise.resolve(typeof value === "function" ? value(args) : value);
          }
          return Promise.resolve([]);
        },
      };
    })();
  `;

  return {
    inject: async (page) => {
      await page.addInitScript(script);
    },
    calls: async (page, command) =>
      await page.evaluate(
        (cmd) =>
          (
            window as unknown as {
              __ipcCalls: { command: string; args: Record<string, unknown> }[];
            }
          ).__ipcCalls.filter((call) => call.command === cmd),
        command,
      ),
    setDialogPath: async (page, path) => {
      await page.evaluate((value) => {
        (window as unknown as { __dialogOpenPath: string | null }).__dialogOpenPath = value;
      }, path);
    },
  };
}
