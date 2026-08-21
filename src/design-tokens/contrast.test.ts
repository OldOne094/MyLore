import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";

/* MISSION-093 — Contrast regression test: parses the design tokens straight
   from `tokens.css` and verifies every text tier meets WCAG AA (≥ 4.5:1 for
   normal text) against both backgrounds, in both themes. Catches any future
   token tweak that silently drops below the line. */

const TOKENS_PATH = join(__dirname, "..", "design-tokens", "tokens.css");

interface ThemeTokens {
  [token: string]: string;
}

function themeTokens(block: string): ThemeTokens {
  const tokens: ThemeTokens = {};
  for (const [, name, value] of block.matchAll(/--([a-z-]+):\s*(#[0-9a-fA-F]{6})/g)) {
    tokens[name] = value;
  }
  return tokens;
}

function luminance(hex: string): number {
  const channels = [0, 2, 4].map((offset) => {
    const c = parseInt(hex.slice(1 + offset, 3 + offset), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

function contrast(foreground: string, background: string): number {
  const l1 = luminance(foreground);
  const l2 = luminance(background);
  const [lighter, darker] = l1 >= l2 ? [l1, l2] : [l2, l1];
  return (lighter + 0.05) / (darker + 0.05);
}

const css = readFileSync(TOKENS_PATH, "utf8");
const lightBlock = css.slice(
  css.indexOf('[data-theme="light"]'),
  css.indexOf('[data-theme="dark"]'),
);
const darkBlock = css.slice(css.indexOf('[data-theme="dark"]'));

const TEXT_TIERS = ["text-primary", "text-secondary", "text-tertiary"];
const BACKGROUNDS = ["bg-base", "bg-surface"];

describe("WCAG AA contrast (MISSION-093)", () => {
  for (const [theme, tokens] of [
    ["light", themeTokens(lightBlock)],
    ["dark", themeTokens(darkBlock)],
  ] as const) {
    describe(`${theme} theme`, () => {
      for (const tier of TEXT_TIERS) {
        for (const background of BACKGROUNDS) {
          it(`${tier} on ${background} is at least 4.5:1`, () => {
            const ratio = contrast(tokens[tier], tokens[background]);
            expect(ratio).toBeGreaterThanOrEqual(4.5);
          });
        }
      }
    });
  }
});
