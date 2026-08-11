/* DESIGN_SYSTEM.md — Status bar: persistent app state + hint area at the bottom
   of the shell. Placeholder values until live data lands in M5+. */

export function StatusBar() {
  return (
    <footer className="flex h-7 shrink-0 items-center justify-between border-t border-border-subtle bg-bg-surface px-5 text-xs text-text-tertiary">
      <span>{NAV_STATUS_LABEL}</span>
      <span className="tabular-nums">{PLACEHOLDER_COUNTS}</span>
    </footer>
  );
}

const NAV_STATUS_LABEL = "v0.1.0";
const PLACEHOLDER_COUNTS = "0 titles";
