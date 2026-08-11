import { describe, expect, it } from "vitest";
import { formatKeyCombo, matchesKeyCombo, parseKeyCombo } from "./keys";

function keyEvent(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent("keydown", init);
}

describe("parseKeyCombo", () => {
  it("treats Mod as platform primary", () => {
    expect(parseKeyCombo("Mod+K", "win")).toEqual({
      key: "k",
      ctrl: true,
      meta: false,
      shift: false,
      alt: false,
    });
    expect(parseKeyCombo("Mod+K", "mac")).toEqual({
      key: "k",
      ctrl: false,
      meta: true,
      shift: false,
      alt: false,
    });
  });

  it("parses explicit modifiers", () => {
    expect(parseKeyCombo("Ctrl+Shift+P", "win")).toEqual({
      key: "p",
      ctrl: true,
      meta: false,
      shift: true,
      alt: false,
    });
  });

  it("normalizes bare keys", () => {
    expect(parseKeyCombo("Escape", "win").key).toBe("escape");
    expect(parseKeyCombo("ArrowUp", "win").key).toBe("arrowup");
  });
});

describe("matchesKeyCombo", () => {
  it("requires the combo's modifiers exactly", () => {
    expect(
      matchesKeyCombo(keyEvent({ key: "k", ctrlKey: true }), parseKeyCombo("Mod+K", "win")),
    ).toBe(true);
    expect(matchesKeyCombo(keyEvent({ key: "k" }), parseKeyCombo("Mod+K", "win"))).toBe(false);
    expect(
      matchesKeyCombo(
        keyEvent({ key: "k", ctrlKey: true, metaKey: true }),
        parseKeyCombo("Mod+K", "win"),
      ),
    ).toBe(false);
  });

  it("matches meta on macOS", () => {
    expect(
      matchesKeyCombo(keyEvent({ key: "k", metaKey: true }), parseKeyCombo("Mod+K", "mac")),
    ).toBe(true);
    expect(
      matchesKeyCombo(keyEvent({ key: "k", ctrlKey: true }), parseKeyCombo("Mod+K", "mac")),
    ).toBe(false);
  });
});

describe("formatKeyCombo", () => {
  it("formats hints per platform", () => {
    expect(formatKeyCombo("Mod+K", "mac")).toBe("⌘K");
    expect(formatKeyCombo("Mod+K", "win")).toBe("Ctrl+K");
    expect(formatKeyCombo("Ctrl+Shift+P", "win")).toBe("Ctrl+Shift+P");
    expect(formatKeyCombo("Escape", "win")).toBe("Esc");
  });
});
