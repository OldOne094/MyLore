import { useTranslation } from "react-i18next";
import { EmptyState } from "@/components/ui";
import { NAV_ITEMS } from "@/navigation";

/* Placeholder pages until each feature ships its real view (MISSION-032).
   Titles and hints come from the i18n resources (MISSION-033). */

function placeholder(path: string) {
  const item = NAV_ITEMS.find((n) => n.path === path);
  if (!item) throw new Error(`unknown navigation path: ${path}`);
  return function PlaceholderPage() {
    const { t } = useTranslation();
    return <EmptyState icon={item.icon} title={t(`nav.${item.key}`)} hint={t(item.hintKey)} />;
  };
}

export { DashboardPage } from "@/features/dashboard/DashboardPage";
export { LibraryPage } from "@/features/library/LibraryPage";
export { SearchPage } from "@/features/search/SearchPage";
export { TrashPage } from "@/features/trash/TrashPage";
export { DiscoverPage } from "@/features/discover/DiscoverPage";
export { CollectionsPage } from "@/features/collections/CollectionsPage";
export { CollectionDetailPage } from "@/features/collections/CollectionDetailPage";
export const ReviewsPage = placeholder("/reviews");
export { StatsPage } from "@/features/stats/StatsPage";
export { CalendarPage } from "@/features/calendar/CalendarPage";
export { RecapPage } from "@/features/recap/RecapPage";
export { SettingsPage } from "@/features/settings/SettingsPage";
