import { useEffect, useState } from "react";
import { AddMediaDialog } from "./AddMediaDialog";
import { useShortcuts } from "@/shortcuts/useShortcuts";
import { OPEN_ADD_MEDIA_EVENT } from "@/shortcuts/map";

/* MISSION-090 — Global add-title dialog. Mounted once in the shell; opens
   with Mod+N or the palette's "Add title" command (which dispatches the
   `mylore:open-add-media` window event, keeping the palette decoupled). */

export function GlobalAddMedia() {
  const [open, setOpen] = useState(false);

  useShortcuts([{ combo: "Mod+N", handler: () => setOpen(true) }]);

  useEffect(() => {
    const handler = () => setOpen(true);
    window.addEventListener(OPEN_ADD_MEDIA_EVENT, handler);
    return () => window.removeEventListener(OPEN_ADD_MEDIA_EVENT, handler);
  }, []);

  // Mounted only while open so the shell can render without a
  // QueryClientProvider (same pattern as QuickCapture).
  if (!open) return null;
  return <AddMediaDialog open onOpenChange={setOpen} />;
}
