import { NavLink } from "react-router";
import { useTranslation } from "react-i18next";
import { NAV_ITEMS } from "@/navigation";
import { useProfile } from "@/profile/ProfileContext";
import { Avatar } from "@/profile/Avatar";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md §6 — Primary nav rail: avatar + brand, icon + text links,
   active state highlighted. Logical layout mirrors in RTL (MISSION-033).
   MISSION-132 adds the user profile avatar. */

export function NavRail() {
  const { t } = useTranslation();
  const { profile } = useProfile();

  return (
    <nav
      aria-label={t("a11y.navTitle")}
      className="flex h-full w-56 shrink-0 flex-col gap-1 border-e border-border-subtle bg-bg-surface p-3"
    >
      <div className="flex items-center gap-2.5 px-2 pb-3 pt-1">
        <Avatar
          displayName={profile.displayName || t("shell.brand")}
          color={profile.avatarColor}
          size={32}
        />
        <div className="min-w-0">
          <p className="truncate text-sm font-semibold text-accent">
            {profile.displayName || t("shell.brand")}
          </p>
        </div>
      </div>
      {NAV_ITEMS.map((item) => (
        <NavLink
          key={item.path}
          to={item.path}
          end={item.path === "/library"}
          className={({ isActive }) =>
            cn(
              "flex select-none items-center gap-3 rounded-sm px-3 py-2 text-sm font-medium transition-colors duration-150 ease-out",
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
