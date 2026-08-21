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

      function handleStore(cmd, args) {
        if (cmd === "plugin:store|load") {
          const rid = nextId++;
          stores.set(rid, { data: {} });
          return Promise.resolve(rid);
        }
        const store = stores.get(args?.rid);
        if (!store) return Promise.resolve(null);
        if (cmd === "plugin:store|get") return Promise.resolve(store.data[args.key] ?? null);
        if (cmd === "plugin:store|set") {
          store.data[args.key] = args.value;
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
