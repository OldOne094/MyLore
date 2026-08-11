import { useEffect, useRef } from "react";
import { matchesKeyCombo, parseKeyCombo } from "./keys";

/* MISSION-036 — Global keyboard shortcut registry. Registers window-level
   keydown listeners for the given combos. Editable targets (inputs, textareas,
   contenteditable) swallow bare-letter shortcuts unless a modifier is held or
   the shortcut opts in with `allowWhileTyping`. */

export interface ShortcutRegistration {
  /** Combo like "Mod+K", "Ctrl+Shift+P", or "Escape". */
  combo: string;
  handler: (event: KeyboardEvent) => void;
  /** Skip this shortcut while disabled (listener stays registered). */
  enabled?: boolean;
  /** Fire even when an editable element has focus and no modifier is held. */
  allowWhileTyping?: boolean;
}

export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

export function useShortcuts(shortcuts: ShortcutRegistration[]): void {
  const shortcutsRef = useRef(shortcuts);

  // Keep the registered combos fresh without writing the ref during render.
  useEffect(() => {
    shortcutsRef.current = shortcuts;
  });

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      for (const shortcut of shortcutsRef.current) {
        if (shortcut.enabled === false) continue;
        const combo = parseKeyCombo(shortcut.combo);
        if (!matchesKeyCombo(event, combo)) continue;
        const typing = isTypingTarget(event.target);
        const hasModifier = combo.ctrl || combo.meta || combo.alt;
        if (typing && !hasModifier && !shortcut.allowWhileTyping) continue;
        event.preventDefault();
        shortcut.handler(event);
        return;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
