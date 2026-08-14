import { Outlet } from "react-router";
import { NavRail } from "./NavRail";
import { TopBar } from "./TopBar";
import { StatusBar } from "./StatusBar";
import { SkipLink } from "./SkipLink";
import { CommandPalette } from "@/features/command-palette/CommandPalette";
import { QuickCapture } from "@/features/library/QuickCapture";

/* App shell (MISSION-032): nav rail + top bar + status bar around the routed
   content. Layout uses logical flow so it mirrors in RTL for Arabic. The skip
   link (MISSION-037) is the first tab stop and targets the content landmark. */

export function AppShell() {
  return (
    <>
      <SkipLink />
      <div className="flex h-screen overflow-hidden bg-bg-base text-text-primary">
        <NavRail />
        <div className="flex min-w-0 flex-1 flex-col">
          <TopBar />
          <main
            id="main-content"
            tabIndex={-1}
            className="min-h-0 flex-1 overflow-auto outline-none"
          >
            <Outlet />
          </main>
          <StatusBar />
        </div>
        <CommandPalette />
        <QuickCapture />
      </div>
    </>
  );
}
