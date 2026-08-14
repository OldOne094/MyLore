import { Monitor, Moon, Sun, Zap, type LucideIcon } from "lucide-react";
import type { TFunction } from "i18next";
import type { NavigateFunction } from "react-router";
import { NAV_ITEMS } from "@/navigation";
import { formatKeyCombo } from "@/shortcuts/keys";
import type { ThemePreference } from "@/themes/theme";

/* MISSION-036 — Command registry for the palette. Commands are assembled with
   the hooks they depend on; the palette only renders and dispatches them. The
   quick-capture action (MISSION-049) opens the popover by dispatching the
   `mylore:open-quick-capture` window event so the palette stays decoupled. */

export type PaletteGroup = "navigation" | "actions";

export interface PaletteCommand {
  id: string;
  group: PaletteGroup;
  label: string;
  keywords?: string[];
  icon: LucideIcon;
  /** Localized keyboard hint shown alongside the action. */
  hint?: string;
  run: () => void;
}

export const PALETTE_GROUPS: PaletteGroup[] = ["navigation", "actions"];

const THEME_ICONS: Record<ThemePreference, LucideIcon> = {
  light: Sun,
  dark: Moon,
  system: Monitor,
};

export function buildPaletteCommands(deps: {
  t: TFunction;
  navigate: NavigateFunction;
  setTheme: (preference: ThemePreference) => void;
}): PaletteCommand[] {
  const { t, navigate, setTheme } = deps;

  const navigation: PaletteCommand[] = NAV_ITEMS.map((item) => ({
    id: `nav:${item.path}`,
    group: "navigation",
    label: t(`nav.${item.key}`),
    keywords: [item.key, "go to", "open"],
    icon: item.icon,
    run: () => navigate(item.path),
  }));

  const actions: PaletteCommand[] = [
    ...(["light", "dark", "system"] as const).map((preference) => ({
      id: `theme:${preference}`,
      group: "actions" as const,
      label: t(`theme.${preference}`),
      keywords: ["theme", "appearance", "color scheme"],
      icon: THEME_ICONS[preference],
      run: () => setTheme(preference),
    })),
    {
      id: "quick:capture",
      group: "actions",
      label: t("quick.open"),
      keywords: ["quick capture", "progress", "mark done", "catch up"],
      icon: Zap,
      hint: formatKeyCombo("Mod+Enter"),
      run: () => window.dispatchEvent(new Event("mylore:open-quick-capture")),
    },
  ];

  return [...navigation, ...actions];
}

export function filterPaletteCommands(commands: PaletteCommand[], query: string): PaletteCommand[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return commands;
  return commands.filter((command) => {
    const haystack = [command.label, ...(command.keywords ?? [])].join(" ").toLowerCase();
    return haystack.includes(needle);
  });
}
