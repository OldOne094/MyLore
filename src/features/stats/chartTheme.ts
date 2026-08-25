/* MISSION-113 — Shared chart theming. Reads colors from CSS custom properties
   (design tokens) so charts follow the active theme + accent automatically.
   No React context dependency — safe for any rendering environment. */

export function useChartTheme() {
  const isDark =
    typeof document !== "undefined" &&
    document.documentElement.getAttribute("data-theme") === "dark";

  return {
    grid: isDark ? "#3a3a44" : "#e5e4e0",
    tick: {
      fill: isDark ? "#a8a7a0" : "#57564f",
      fontSize: 11,
    },
    tooltip: {
      backgroundColor: isDark ? "#24242a" : "#ffffff",
      border: `1px solid ${isDark ? "#3a3a44" : "#e5e4e0"}`,
      borderRadius: 8,
      color: isDark ? "#f2f1ef" : "#1c1b1a",
      fontSize: 12,
    },
    accent: "var(--accent)",
    accentSoft: "var(--accent-soft)",
    ok: "var(--ok)",
    info: "var(--info)",
    warn: "var(--warn)",
    danger: "var(--danger)",
    isRTL: typeof document !== "undefined" && document.documentElement.dir === "rtl",
  };
}

/** Resolve a CSS variable to its computed value for SVG fill/stroke attrs. */
export function cssVar(name: string): string {
  if (typeof window === "undefined") return "#b4541f";
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#b4541f";
}
