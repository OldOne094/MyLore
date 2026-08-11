/* MISSION-036 — Keyboard shortcut primitives: platform-aware combos
   ("Mod+K" = Cmd on macOS, Ctrl elsewhere), matching against KeyboardEvents,
   and hint formatting for UI (⌘K / Ctrl+K). Framework-free. */

export type Platform = "mac" | "win" | "linux";

export interface KeyCombo {
  key: string;
  ctrl: boolean;
  meta: boolean;
  shift: boolean;
  alt: boolean;
}

export function platform(): Platform {
  if (typeof navigator === "undefined") return "linux";
  const ua = navigator.userAgent;
  if (/Mac/i.test(ua)) return "mac";
  if (/Windows/i.test(ua)) return "win";
  return "linux";
}

const KEY_LABELS: Record<string, string> = {
  " ": "Space",
  backspace: "Backspace",
  delete: "Delete",
  enter: "Enter",
  escape: "Esc",
  tab: "Tab",
  arrowup: "↑",
  arrowdown: "↓",
  arrowleft: "←",
  arrowright: "→",
};

function normalizeKey(key: string): string {
  return key.length === 1 ? key.toLowerCase() : key.toLowerCase();
}

/** Parse a combo string like "Mod+K", "Ctrl+Shift+P", or "Escape". */
export function parseKeyCombo(combo: string, p: Platform = platform()): KeyCombo {
  const parts = combo.split("+").map((part) => part.trim().toLowerCase());
  const key = normalizeKey(parts.pop() ?? "");
  const usesMod = parts.includes("mod");
  return {
    key,
    ctrl: usesMod ? p !== "mac" : parts.includes("ctrl"),
    meta: usesMod ? p === "mac" : parts.includes("meta"),
    shift: parts.includes("shift"),
    alt: parts.includes("alt"),
  };
}

/** True when the event carries exactly the combo's key + required modifiers. */
export function matchesKeyCombo(event: KeyboardEvent, combo: KeyCombo): boolean {
  if (event.key.toLowerCase() !== combo.key) return false;
  if (combo.ctrl !== event.ctrlKey) return false;
  if (combo.meta !== event.metaKey) return false;
  if (combo.shift !== event.shiftKey) return false;
  if (combo.alt !== event.altKey) return false;
  return true;
}

/** Human-readable hint: "⌘K" on macOS, "Ctrl+K" elsewhere. */
export function formatKeyCombo(combo: string, p: Platform = platform()): string {
  const parsed = parseKeyCombo(combo, p);
  const key = KEY_LABELS[parsed.key] ?? parsed.key.toUpperCase();

  if (p === "mac") {
    const pieces: string[] = [];
    if (parsed.ctrl) pieces.push("⌃");
    if (parsed.meta) pieces.push("⌘");
    if (parsed.alt) pieces.push("⌥");
    if (parsed.shift) pieces.push("⇧");
    return [...pieces, key].join("");
  }

  const pieces: string[] = [];
  if (parsed.ctrl) pieces.push("Ctrl");
  if (parsed.meta) pieces.push("Meta");
  if (parsed.alt) pieces.push("Alt");
  if (parsed.shift) pieces.push("Shift");
  return [...pieces, key].join("+");
}
