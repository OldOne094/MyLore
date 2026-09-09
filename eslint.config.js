import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import prettier from "eslint-config-prettier";

export default tseslint.config(
  {
    // `.freebuff` holds local worktree scratch copies of the repo — its nested
    // tsconfigs would break the TS parser and it must never be linted.
    ignores: ["dist", "dist-ssr", "src-tauri/target", "node_modules", ".freebuff/**"],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
    },
  },
  {
    files: ["scripts/**/*.mjs"],
    languageOptions: {
      globals: globals.node,
    },
  },
  {
    // UI primitives intentionally re-export Radix subcomponents and hooks from
    // one module (e.g. Dialog = Root/Trigger/Content) — fast refresh doesn't
    // apply to this split, so the only-export-components rule is disabled here.
    files: ["src/components/ui/**/*.{ts,tsx}"],
    rules: {
      "react-refresh/only-export-components": "off",
    },
  },
  prettier,
);
