import { Navigate, type RouteObject } from "react-router";
import { AppShell } from "@/components/shell/AppShell";
import {
  CalendarPage,
  CollectionsPage,
  DiscoverPage,
  LibraryPage,
  ReviewsPage,
  SearchPage,
  SettingsPage,
  StatsPage,
  TrashPage,
} from "@/features";
import { MediaDetailPage } from "@/features/library/MediaDetailPage";
import { NAV_ITEMS } from "@/navigation";

/* Route table (MISSION-032). Shared by the app router and tests (memory router). */

export const appRoutes: RouteObject[] = [
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <Navigate to={NAV_ITEMS[0].path} replace /> },
      { path: "library", element: <LibraryPage /> },
      { path: "library/:id", element: <MediaDetailPage /> },
      { path: "search", element: <SearchPage /> },
      { path: "discover", element: <DiscoverPage /> },
      { path: "collections", element: <CollectionsPage /> },
      { path: "reviews", element: <ReviewsPage /> },
      { path: "stats", element: <StatsPage /> },
      { path: "calendar", element: <CalendarPage /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "trash", element: <TrashPage /> },
    ],
  },
];
