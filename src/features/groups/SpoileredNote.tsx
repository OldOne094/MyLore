/* MISSION-117 — One discussion note, hidden while its author is further along
   than the reader (progress-gated spoilers, MISSION-108's UX).

   While gated the body is not rendered at all — a blurred element would still be
   in the DOM, selectable and readable by assistive tech, which is not what
   "hidden" means to the person who has not reached that point yet. Revealing is
   per note and lasts for the session; catching up lifts the gate on its own. */

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui";
import type { ThreadNote } from "@/features/groups/alignment";

export interface SpoileredNoteProps {
  note: ThreadNote;
  authorName: string;
  /** The author's recorded progress on this work (> yours while gated). */
  authorProgress: number;
  gated: boolean;
}

function formatStamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function SpoileredNote({ note, authorName, authorProgress, gated }: SpoileredNoteProps) {
  const { t } = useTranslation();
  const [revealed, setRevealed] = useState(false);
  const hidden = gated && !revealed;

  return (
    <li
      data-testid={`group-note-${note.id}`}
      className="flex flex-col gap-1.5 border-t border-border-subtle pt-3 first:border-t-0 first:pt-0"
    >
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-sm font-medium text-text-primary">
          {authorName || t("groupsPage.unknownAuthor")}
        </span>
        {note.at ? (
          <span className="text-xs text-text-tertiary">{formatStamp(note.at)}</span>
        ) : null}
      </div>

      {hidden ? (
        <div className="flex flex-wrap items-center justify-between gap-2 rounded-sm bg-bg-hover px-2.5 py-2">
          <span className="text-xs text-text-tertiary">
            {t("groupsPage.spoilerWatermark", { name: authorName, progress: authorProgress })}
          </span>
          <Button size="sm" variant="ghost" onClick={() => setRevealed(true)}>
            {t("groupsPage.showAnyway")}
          </Button>
        </div>
      ) : (
        <p className="text-sm whitespace-pre-wrap text-text-secondary">{note.body}</p>
      )}
    </li>
  );
}
