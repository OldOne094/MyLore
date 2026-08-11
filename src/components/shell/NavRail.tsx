import { NavLink } from "react-router";
import { useTranslation } from "react-i18next";
import { NAV_ITEMS } from "@/navigation";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Primary nav rail: icon + text, active state highlighted.
   Logical layout mirrors automatically in RTL (MISSION-033). */

export function NavRail() {
  const { t } = useTranslation();

  return (
    <nav
      aria-label={t("nav.library")}
      className="flex h-full w-56 shrink-0 flex-col gap-1 border-e border-border-subtle bg-bg-surface p-3"
    >
      <span className="px-3 pb-2 pt-1 text-sm font-semibold text-accent">{t("shell.brand")}</span>
      {NAV_ITEMS.map((item) => (
        <NavLink
          key={item.path}
          to={item.path}
          end={item.path === "/library"}
          className={({ isActive }) =>
            cn(
              "flex items-center gap-3 rounded-sm px-3 py-2 text-sm font-medium transition-colors duration-150 ease-out",
              isActive
                ? "bg-accent-soft text-accent"
                : "text-text-secondary hover:bg-bg-hover hover:text-text-primary",
            )
          }
        >
          <item.icon size={18} aria-hidden="true" />
          {t(`nav.${item.key}`)}
        </NavLink>
      ))}
    </nav>
  );
}
