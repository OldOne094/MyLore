import {
  BarChart3,
  Calendar,
  Compass,
  FolderHeart,
  LayoutDashboard,
  LibraryBig,
  Search,
  Settings,
  Star,
  Trash2,
  Trophy,
  type LucideIcon,
} from "lucide-react";

/* App navigation (MISSION-032, keys MISSION-033). Single place for paths and
   i18n keys so the nav rail, top bar and routes always agree. Text lives in
   the translation resources under `nav.*`. */

export interface NavItem {
  path: string;
  /** i18n key for the label/title (nav.<key>). */
  key: string;
  icon: LucideIcon;
  /** i18n key for the empty-state hint (nav.hint_<key>). */
  hintKey: string;
}

export const NAV_ITEMS: NavItem[] = [
  { path: "/dashboard", key: "dashboard", icon: LayoutDashboard, hintKey: "nav.hint_dashboard" },
  { path: "/library", key: "library", icon: LibraryBig, hintKey: "nav.hint_library" },
  { path: "/search", key: "search", icon: Search, hintKey: "nav.hint_search" },
  { path: "/discover", key: "discover", icon: Compass, hintKey: "nav.hint_discover" },
  { path: "/collections", key: "collections", icon: FolderHeart, hintKey: "nav.hint_collections" },
  { path: "/reviews", key: "reviews", icon: Star, hintKey: "nav.hint_reviews" },
  { path: "/stats", key: "stats", icon: BarChart3, hintKey: "nav.hint_stats" },
  { path: "/calendar", key: "calendar", icon: Calendar, hintKey: "nav.hint_calendar" },
  { path: "/recap", key: "recap", icon: Trophy, hintKey: "nav.hint_recap" },
  { path: "/settings", key: "settings", icon: Settings, hintKey: "nav.hint_settings" },
  { path: "/trash", key: "trash", icon: Trash2, hintKey: "nav.hint_trash" },
];
