/* MISSION-117 — Reading-groups view logic, kept pure so the page can stay
   presentational and this file can be tested without React.

   Two things live here that encode product truth:

   1. The **alignment matrix** — a reading group exists to show where everyone is
      on the *same* work, so shelves are pivoted from "one member's list" into
      rows of works carrying a per-member entry.
   2. The **spoiler gate** — a member's note is hidden while they are further
      along than you are. It uses each member's real shelf progress, so it lifts
      by itself as you catch up; nothing about a note's content is guessed.

   Note ids carry their author and time (see `encodeNoteId`), because the
   conflict-free document only stores id → body. */

import type { GroupMemberView, GroupNoteEntry, GroupNoteView, GroupShelfEntryView } from "@/api";

/** Note-id shape: `n:{13-digit ms}:{authorId}:{uuid}` — fixed width so the
    document's id-ordering is chronological, and the author survives the sync. */
const NOTE_ID_PREFIX = "n";
const NOTE_ID_TIME_DIGITS = 13;

/** A work with every member's shelf entry beside it. */
export interface AlignmentWork {
  work_key: string;
  title: string;
  content_type: string;
  /** Shelf entry per member id; a missing key means that member hasn't shelved it. */
  by_member: Record<string, GroupShelfEntryView>;
}

/** One note as the thread renders it, from either note store. */
export interface ThreadNote {
  id: string;
  author_id: string;
  body: string;
  /** ISO timestamp, or null when the source carries none. */
  at: string | null;
}

/** Progress a member has recorded on a work (0 when they have no entry). */
export function progressOf(work: AlignmentWork, memberId: string): number {
  return work.by_member[memberId]?.progress ?? 0;
}

/** Whether a member's entry is a work anyone can discuss yet. */
export function hasShelved(work: AlignmentWork, memberId: string): boolean {
  return work.by_member[memberId] !== undefined;
}

/**
 * Whether `authorId`'s note on `work` may spoil `readerId`.
 *
 * True when the author is *further along* than the reader. A reader with no
 * entry is at 0, so anything from a member who has started is gated — which is
 * the honest default: the reader hasn't told us they're past that point.
 */
export function maySpoil(work: AlignmentWork, authorId: string, readerId: string): boolean {
  if (authorId === readerId) return false;
  return progressOf(work, authorId) > progressOf(work, readerId);
}

/**
 * Pivot the group's shelf rows into work-major rows carrying every member's
 * entry, ordered by title so the group reads the same list on every device.
 */
export function buildAlignment(
  shelf: GroupShelfEntryView[],
  members: GroupMemberView[],
): AlignmentWork[] {
  const known = new Set(members.map((member) => member.member_id));
  const rows = new Map<string, AlignmentWork>();
  for (const entry of shelf) {
    const row = rows.get(entry.work_key) ?? {
      work_key: entry.work_key,
      title: entry.title,
      content_type: entry.content_type,
      by_member: {},
    };
    // Rows from a member who left the group no longer belong to it.
    if (known.has(entry.member_id)) {
      row.by_member[entry.member_id] = entry;
    }
    rows.set(entry.work_key, row);
  }
  return [...rows.values()].sort(
    (a, b) =>
      a.title.localeCompare(b.title, undefined, { sensitivity: "base" }) ||
      a.work_key.localeCompare(b.work_key),
  );
}

/** Encode a thread note id, stamping creator and time into the id itself. */
export function encodeNoteId(
  authorId: string,
  now: number = Date.now(),
  uuid: string = freshId(),
): string {
  const time = String(Math.max(0, Math.trunc(now))).padStart(NOTE_ID_TIME_DIGITS, "0");
  return [NOTE_ID_PREFIX, time, authorId, uuid].join(":");
}

/** Recover the author (and time, when stamped) from a note id. */
export function decodeNoteId(id: string): { author_id: string; at: string | null } {
  const parts = id.split(":");
  const stamped =
    parts.length >= 4 &&
    parts[0] === NOTE_ID_PREFIX &&
    parts[1].length === NOTE_ID_TIME_DIGITS &&
    /^\d+$/.test(parts[1]);
  if (!stamped) {
    // An id from elsewhere: keep the body, admit we don't know who wrote it.
    return { author_id: "", at: null };
  }
  const millis = Number(parts[1]);
  return {
    author_id: parts[2],
    at: Number.isFinite(millis) ? new Date(millis).toISOString() : null,
  };
}

/** Notes from the synced document, whose ids carry author + time. */
export function notesFromDocument(entries: GroupNoteEntry[]): ThreadNote[] {
  return entries.map((entry) => {
    const { author_id, at } = decodeNoteId(entry.note_id);
    return { id: entry.note_id, author_id, body: entry.body, at };
  });
}

/** Notes from the always-available local table. */
export function notesFromTable(rows: GroupNoteView[]): ThreadNote[] {
  return [...rows]
    .sort((a, b) => a.created_at.localeCompare(b.created_at) || a.id.localeCompare(b.id))
    .map((row) => ({
      id: row.id,
      author_id: row.author_id,
      body: row.body,
      at: row.created_at,
    }));
}

/** The display name to show for a member id, falling back to the raw id. */
export function memberName(members: GroupMemberView[], memberId: string): string {
  if (memberId === "") return "";
  return members.find((member) => member.member_id === memberId)?.display_name || memberId;
}

function freshId(): string {
  const cryptoApi = globalThis.crypto as Crypto | undefined;
  return cryptoApi?.randomUUID ? cryptoApi.randomUUID() : Math.random().toString(36).slice(2);
}
