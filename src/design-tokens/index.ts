/* MyLore design tokens for JS/TS (charts, inline styles, dynamic color work).
   The CSS source of truth is tokens.css; keep these values in sync. */

export const colors = {
  light: {
    base: "#FAFAF9",
    surface: "#FFFFFF",
    raised: "#FFFFFF",
    hover: "#F1F1EF",
    borderSubtle: "#E5E4E0",
    borderStrong: "#D4D3CE",
    textPrimary: "#1C1B1A",
    textSecondary: "#57564F",
    textTertiary: "#85847C",
    accent: "#B4541F",
    accentHover: "#9C4719",
    accentSoft: "#F7E7DC",
    ok: "#2F7D32",
    warn: "#9A6700",
    danger: "#C62828",
    info: "#1565C0",
  },
  dark: {
    base: "#141417",
    surface: "#1C1C21",
    raised: "#24242A",
    hover: "#2A2A31",
    borderSubtle: "#2E2E36",
    borderStrong: "#3A3A44",
    textPrimary: "#F2F1EF",
    textSecondary: "#A8A7A0",
    textTertiary: "#71716C",
    accent: "#E08A4C",
    accentHover: "#EEA060",
    accentSoft: "#3A2A1F",
    ok: "#7BC47F",
    warn: "#D9A441",
    danger: "#E57373",
    info: "#64B5F6",
  },
} as const;

export const spacing = {
  1: 4,
  2: 8,
  3: 12,
  4: 16,
  5: 24,
  6: 32,
  7: 48,
} as const;

export const radius = {
  sm: 6,
  md: 10,
  lg: 16,
  full: 999,
} as const;

export const fontSize = {
  "2xs": 12,
  sm: 13,
  base: 14,
  md: 16,
  lg: 20,
  xl: 28,
  "2xl": 36,
} as const;

export const duration = {
  fast: 120,
  base: 160,
  slow: 200,
} as const;
