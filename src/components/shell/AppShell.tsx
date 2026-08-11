import { Outlet } from "react-router";
import { NavRail } from "./NavRail";
import { TopBar } from "./TopBar";
import { StatusBar } from "./StatusBar";
import { CommandPalette } from "@/features/command-palette/CommandPalette";

/* App shell (MISSION-032): nav rail + top bar + status bar around the routed
   content. Layout uses logical flow so it mirrors in RTL for Arabic. */

export function AppShell() {
  return (
    <div className="flex h-screen overflow-hidden bg-bg-base text-text-primary">
      <NavRail />
      <div className="flex min-w-0 flex-1 flex-col">
        <TopBar />
        <main className="min-h-0 flex-1 overflow-auto">
          <Outlet />
        </main>
        <StatusBar />
      </div>
      <CommandPalette />
    </div>
  );
}
