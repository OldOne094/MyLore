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

export const LibraryPage = placeholder("/library");
export const SearchPage = placeholder("/search");
export const DiscoverPage = placeholder("/discover");
export const CollectionsPage = placeholder("/collections");
export const ReviewsPage = placeholder("/reviews");
export const StatsPage = placeholder("/stats");
export const CalendarPage = placeholder("/calendar");
export { SettingsPage } from "@/features/settings/SettingsPage";
