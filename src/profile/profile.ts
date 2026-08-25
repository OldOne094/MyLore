/* MISSION-132 — User profile model. Purely cosmetic (display name + avatar
   color); stored client-side alongside other preferences. No backend needed
   since the profile never leaves the device (local-first contract). */

export const AVATAR_COLORS = [
  "#b4541f", // terracotta (default accent)
  "#2563eb", // ocean blue
  "#7c3aed", // violet
  "#047857", // emerald
  "#be123c", // rose
  "#b45309", // amber
  "#0e7490", // teal
  "#6d28d9", // deep purple
] as const;

export type AvatarColor = (typeof AVATAR_COLORS)[number];

export interface UserProfile {
  displayName: string;
  avatarColor: string;
}

export const DEFAULT_PROFILE: UserProfile = {
  displayName: "",
  avatarColor: AVATAR_COLORS[0],
};

/** Extract initials from a display name (up to 2 chars). */
export function initials(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return "?";
  const parts = trimmed.split(/\s+/);
  if (parts.length >= 2) {
    return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
  }
  return trimmed.slice(0, 2).toUpperCase();
}
