import type { StatsView } from "@/api";
import type { UserProfile } from "@/profile/profile";

/* MISSION-132c — Shareable stats card rendered on Canvas and exported as PNG.
   Draws a beautiful summary card with the user's accent color, key stats,
   and avatar — no external libraries needed. */

interface CardData {
  stats: StatsView;
  profile: UserProfile;
  year: number;
  topGenre?: string;
}

const W = 800;
const H = 450;

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

export async function exportStatsCard(data: CardData): Promise<void> {
  const canvas = document.createElement("canvas");
  canvas.width = W * 2; // 2x for retina quality
  canvas.height = H * 2;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.scale(2, 2);

  // Background gradient
  const grad = ctx.createLinearGradient(0, 0, 0, H);
  grad.addColorStop(0, data.profile.avatarColor);
  grad.addColorStop(1, shade(data.profile.avatarColor, -30));
  roundRect(ctx, 0, 0, W, H, 24);
  ctx.fillStyle = grad;
  ctx.fill();

  // Subtle overlay pattern
  ctx.fillStyle = "rgba(255,255,255,0.04)";
  for (let i = 0; i < 6; i++) {
    ctx.beginPath();
    ctx.arc(W - 60 - i * 80, H - 40, 60 + i * 30, 0, Math.PI * 2);
    ctx.fill();
  }

  // Title
  ctx.fillStyle = "rgba(255,255,255,0.85)";
  ctx.font = "600 16px 'Segoe UI', sans-serif";
  ctx.textBaseline = "top";
  ctx.fillText("MyLore", 36, 28);

  ctx.font = "700 28px 'Segoe UI', sans-serif";
  ctx.fillStyle = "#ffffff";
  ctx.fillText(`Your Library — ${data.year}`, 36, 52);

  // Avatar
  const avatarX = W - 120;
  const avatarY = 32;
  const avatarR = 28;
  ctx.beginPath();
  ctx.arc(avatarX + avatarR, avatarY + avatarR, avatarR, 0, Math.PI * 2);
  ctx.fillStyle = "rgba(255,255,255,0.25)";
  ctx.fill();

  const name = data.profile.displayName || "Reader";
  const initialsText = getInitials(name);
  ctx.font = "700 20px 'Segoe UI', sans-serif";
  ctx.fillStyle = "#ffffff";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(initialsText, avatarX + avatarR, avatarY + avatarR);

  // Stats grid
  const statsY = 140;
  const colW = (W - 72) / 3;
  const padding = 36;
  const stats = [
    { label: "Titles", value: String(data.stats.total) },
    { label: "Completed", value: String(data.stats.completed_media) },
    {
      label: "Hours",
      value: data.stats.consumed_hours > 0 ? String(data.stats.consumed_hours) : "—",
    },
    {
      label: "Avg Rating",
      value: data.stats.avg_rating != null ? `${data.stats.avg_rating}/10` : "—",
    },
    { label: "Favorites", value: String(data.stats.favorites) },
    {
      label: "Completion",
      value: data.stats.completion_rate != null ? `${data.stats.completion_rate}%` : "—",
    },
  ];

  // Stat cards background
  roundRect(ctx, padding, statsY, W - padding * 2, 160, 12);
  ctx.fillStyle = "rgba(0,0,0,0.15)";
  ctx.fill();

  stats.forEach((stat, i) => {
    const col = i % 3;
    const row = Math.floor(i / 3);
    const x = padding + 20 + col * colW;
    const y = statsY + 22 + row * 68;

    ctx.textAlign = "start";
    ctx.textBaseline = "alphabetic";
    ctx.fillStyle = "rgba(255,255,255,0.6)";
    ctx.font = "500 12px 'Segoe UI', sans-serif";
    ctx.fillText(stat.label.toUpperCase(), x, y);

    ctx.fillStyle = "#ffffff";
    ctx.font = "700 26px 'Segoe UI', sans-serif";
    ctx.fillText(stat.value, x, y + 32);
  });

  // Top genre chip
  if (data.topGenre) {
    const chipY = statsY + 170;
    const chipText = `Top genre: ${data.topGenre}`;
    ctx.font = "500 14px 'Segoe UI', sans-serif";
    const tw = ctx.measureText(chipText).width;
    roundRect(ctx, padding, chipY, tw + 24, 32, 16);
    ctx.fillStyle = "rgba(255,255,255,0.18)";
    ctx.fill();
    ctx.fillStyle = "#ffffff";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(chipText, padding + (tw + 24) / 2, chipY + 16);
  }

  // Footer
  ctx.textAlign = "end";
  ctx.textBaseline = "bottom";
  ctx.fillStyle = "rgba(255,255,255,0.45)";
  ctx.font = "400 11px 'Segoe UI', sans-serif";
  ctx.fillText("Tracked with MyLore", W - 36, H - 20);

  // Export
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!blob) return;

  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `mylore-stats-${data.year}.png`;
  link.click();
  URL.revokeObjectURL(url);
}

function getInitials(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return "?";
  const parts = trimmed.split(/\s+/);
  if (parts.length >= 2) return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
  return trimmed.slice(0, 2).toUpperCase();
}

/** Darken/lighten a hex color by amount (-100..100). */
function shade(hex: string, amount: number): string {
  const num = parseInt(hex.replace("#", ""), 16);
  const r = Math.max(0, Math.min(255, ((num >> 16) & 0xff) + amount));
  const g = Math.max(0, Math.min(255, ((num >> 8) & 0xff) + amount));
  const b = Math.max(0, Math.min(255, (num & 0xff) + amount));
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, "0")}`;
}
