/* MISSION-090 — The complete shortcut map, in one place so the help dialog,
   the palette hints and the registrations never drift apart. */

export interface ShortcutMapEntry {
  id: string;
  combo: string;
  /** i18n key under `shortcuts.` */
  labelKey: string;
}

export const SHORTCUTS: ShortcutMapEntry[] = [
  { id: "palette", combo: "Mod+K", labelKey: "palette" },
  { id: "quickCapture", combo: "Mod+Enter", labelKey: "quickCapture" },
  { id: "addTitle", combo: "Mod+N", labelKey: "addTitle" },
  { id: "help", combo: "?", labelKey: "help" },
];

/** Window event that opens the global add-title dialog. */
export const OPEN_ADD_MEDIA_EVENT = "mylore:open-add-media";
/** Window event that opens the shortcuts help dialog. */
export const OPEN_SHORTCUTS_EVENT = "mylore:open-shortcuts";
