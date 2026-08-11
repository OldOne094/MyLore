import {
  BarChart3,
  Calendar,
  Compass,
  FolderHeart,
  LibraryBig,
  Search,
  Settings,
  Star,
  type LucideIcon,
} from "lucide-react";

/* App navigation (MISSION-032). Single place for paths/labels/icons so the nav
   rail, top bar and routes always agree. */

export interface NavItem {
  path: string;
  label: string;
  icon: LucideIcon;
  pageTitle: string;
  hint: string;
}

export const NAV_ITEMS: NavItem[] = [
  {
    path: "/library",
    label: "Library",
    icon: LibraryBig,
    pageTitle: "Library",
    hint: "Your tracked titles appear here as you add them.",
  },
  {
    path: "/search",
    label: "Search",
    icon: Search,
    pageTitle: "Search",
    hint: "Find titles in your library or add new ones.",
  },
  {
    path: "/discover",
    label: "Discover",
    icon: Compass,
    pageTitle: "Discover",
    hint: "Explore seasonal charts and recommendations.",
  },
  {
    path: "/collections",
    label: "Collections",
    icon: FolderHeart,
    pageTitle: "Collections",
    hint: "Group titles into smart and manual collections.",
  },
  {
    path: "/reviews",
    label: "Reviews",
    icon: Star,
    pageTitle: "Reviews",
    hint: "Write and manage your reviews here.",
  },
  {
    path: "/stats",
    label: "Stats",
    icon: BarChart3,
    pageTitle: "Stats",
    hint: "Time watched, pages read and your ratings distribution.",
  },
  {
    path: "/calendar",
    label: "Calendar",
    icon: Calendar,
    pageTitle: "Calendar",
    hint: "Airing schedules and upcoming release dates.",
  },
  {
    path: "/settings",
    label: "Settings",
    icon: Settings,
    pageTitle: "Settings",
    hint: "Theme, language, data and provider preferences.",
  },
];
