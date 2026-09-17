import { describe, expect, it } from "vitest";
import type { GroupMemberView, GroupNoteEntry, GroupNoteView, GroupShelfEntryView } from "@/api";
import {
  buildAlignment,
  decodeNoteId,
  encodeNoteId,
  memberName,
  maySpoil,
  notesFromDocument,
  notesFromTable,
  progressOf,
} from "./alignment";

function member(member_id: string, display_name: string): GroupMemberView {
  return { member_id, display_name, role: "member", joined_at: "2026-01-01T00:00:00Z" };
}

function shelf(
  member_id: string,
  work_key: string,
  title: string,
  progress: number,
): GroupShelfEntryView {
  return {
    group_id: "g-1",
    member_id,
    work_key,
    title,
    content_type: "book",
    status: progress > 0 ? "in_progress" : "planned",
    progress,
    updated_at: "2026-01-02T00:00:00Z",
  };
}

const MEMBERS = [member("m-me", "Me"), member("m-nour", "Nour")];

describe("buildAlignment", () => {
  it("pivots shelves into one row per work with a per-member entry", () => {
    const rows = buildAlignment(
      [shelf("m-me", "h:b", "Berserk", 3), shelf("m-nour", "h:b", "Berserk", 12)],
      MEMBERS,
    );
    expect(rows).toHaveLength(1);
    expect(rows[0].work_key).toBe("h:b");
    expect(rows[0].by_member["m-me"].progress).toBe(3);
    expect(rows[0].by_member["m-nour"].progress).toBe(12);
  });

  it("orders works by title and drops rows from members who left", () => {
    const rows = buildAlignment(
      [
        shelf("m-me", "h:z", "Zeta", 1),
        shelf("m-me", "h:a", "alpha", 1),
        shelf("m-gone", "h:a", "alpha", 5),
      ],
      MEMBERS,
    );
    expect(rows.map((row) => row.title)).toEqual(["alpha", "Zeta"]);
    expect(rows[0].by_member["m-gone"]).toBeUndefined();
  });

  it("treats a member without an entry as being at zero", () => {
    const rows = buildAlignment([shelf("m-nour", "h:b", "Berserk", 12)], MEMBERS);
    expect(progressOf(rows[0], "m-me")).toBe(0);
  });
});

describe("maySpoil", () => {
  const rows = buildAlignment(
    [shelf("m-me", "h:b", "Berserk", 10), shelf("m-nour", "h:b", "Berserk", 40)],
    MEMBERS,
  );

  it("gates a member who is further along than me", () => {
    expect(maySpoil(rows[0], "m-nour", "m-me")).toBe(true);
  });

  it("never gates my own notes", () => {
    expect(maySpoil(rows[0], "m-me", "m-me")).toBe(false);
  });

  it("lifts the gate once I catch up", () => {
    const caughtUp = buildAlignment(
      [shelf("m-me", "h:b", "Berserk", 40), shelf("m-nour", "h:b", "Berserk", 40)],
      MEMBERS,
    );
    expect(maySpoil(caughtUp[0], "m-nour", "m-me")).toBe(false);
  });

  it("gates a member who has started when I have not shelved the work", () => {
    const behind = buildAlignment([shelf("m-nour", "h:b", "Berserk", 1)], MEMBERS);
    expect(maySpoil(behind[0], "m-nour", "m-me")).toBe(true);
  });
});

describe("note ids", () => {
  it("round-trips the author and time from the id alone", () => {
    const id = encodeNoteId("m-nour", Date.parse("2026-03-04T05:06:07Z"), "uuid-1");
    expect(id).toBe("n:1772600767000:m-nour:uuid-1");
    expect(decodeNoteId(id)).toEqual({
      author_id: "m-nour",
      at: "2026-03-04T05:06:07.000Z",
    });
  });

  it("pads the timestamp so ids sort chronologically", () => {
    expect(encodeNoteId("m-me", 1_000, "a")).toBe("n:0000000001000:m-me:a");
  });

  it("orders chronologically because the timestamp is fixed width", () => {
    const older = encodeNoteId("m-me", 1_000, "a");
    const newer = encodeNoteId("m-me", 2_000, "b");
    expect([newer, older].sort()).toEqual([older, newer]);
  });

  it("admits it cannot attribute an id from elsewhere", () => {
    expect(decodeNoteId("n-1")).toEqual({ author_id: "", at: null });
    expect(decodeNoteId("n:12:m-nour:uuid")).toEqual({ author_id: "", at: null });
  });
});

describe("thread sources", () => {
  it("reads author and time out of document notes", () => {
    const entries: GroupNoteEntry[] = [
      { note_id: encodeNoteId("m-nour", 5_000, "a"), body: "hello" },
    ];
    expect(notesFromDocument(entries)).toEqual([
      {
        id: "n:0000000005000:m-nour:a",
        author_id: "m-nour",
        body: "hello",
        at: "1970-01-01T00:00:05.000Z",
      },
    ]);
  });

  it("sorts table notes oldest first", () => {
    const row = (id: string, created_at: string): GroupNoteView => ({
      id,
      group_id: "g-1",
      work_key: "h:b",
      author_id: "m-me",
      body: id,
      created_at,
      updated_at: created_at,
    });
    const notes = notesFromTable([
      row("b", "2026-02-01T00:00:00Z"),
      row("a", "2026-01-01T00:00:00Z"),
    ]);
    expect(notes.map((note) => note.id)).toEqual(["a", "b"]);
  });
});

describe("memberName", () => {
  it("falls back to the raw id and stays empty for an unknown author", () => {
    expect(memberName(MEMBERS, "m-nour")).toBe("Nour");
    expect(memberName(MEMBERS, "m-ghost")).toBe("m-ghost");
    expect(memberName(MEMBERS, "")).toBe("");
  });
});
