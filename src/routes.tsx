import { Navigate, type RouteObject } from "react-router";
import { AppShell } from "@/components/shell/AppShell";
import {
  CalendarPage,
  CollectionDetailPage,
  CollectionsPage,
  DashboardPage,
  DiscoverPage,
  LibraryPage,
  RecapPage,
  ReviewsPage,
  SearchPage,
  SettingsPage,
  StatsPage,
  TrashPage,
} from "@/features";
import { MediaDetailPage } from "@/features/library/MediaDetailPage";
import { HealthGate } from "@/features/recovery/RecoveryScreen";
import { NAV_ITEMS } from "@/navigation";

/* Route table (MISSION-032). Shared by the app router and tests (memory router).
   The HealthGate (MISSION-088) swaps the whole shell for the recovery screen
   when the database failed its startup integrity check. */

export const appRoutes: RouteObject[] = [
  {
    path: "/",
    element: (
      <HealthGate>
        <AppShell />
      </HealthGate>
    ),
    children: [
      { index: true, element: <Navigate to={NAV_ITEMS[0].path} replace /> },
      { path: "dashboard", element: <DashboardPage /> },
      { path: "library", element: <LibraryPage /> },
      { path: "library/:id", element: <MediaDetailPage /> },
      { path: "search", element: <SearchPage /> },
      { path: "discover", element: <DiscoverPage /> },
      { path: "collections", element: <CollectionsPage /> },
      { path: "collections/:collectionId", element: <CollectionDetailPage /> },
      { path: "reviews", element: <ReviewsPage /> },
      { path: "stats", element: <StatsPage /> },
      { path: "calendar", element: <CalendarPage /> },
      { path: "recap", element: <RecapPage /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "trash", element: <TrashPage /> },
    ],
  },
];
