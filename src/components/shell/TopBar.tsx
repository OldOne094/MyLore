import { useState } from "react";
import { Search } from "lucide-react";
import { useLocation, useNavigate, useSearchParams } from "react-router";
import { useTranslation } from "react-i18next";
import { NAV_ITEMS } from "@/navigation";
import { THEME_CHOICES } from "@/themes/preferences";
import { useTheme } from "@/themes/useTheme";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { cn } from "@/lib/cn";

/* DESIGN_SYSTEM.md — Top bar: current page title + global actions (local
   search, locale, theme). Fully translated (MISSION-033). The search field
   navigates to /search?q= (MISSION-043); while on the search page it mirrors
   the URL query so results and the box stay in sync. The input is remounted
   (via `key`) whenever the URL query changes so no effect is needed to sync. */

function HeaderSearch() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const onSearchPage = location.pathname === "/search";
  const urlQuery = onSearchPage ? (searchParams.get("q") ?? "") : "";
  const [draft, setDraft] = useState(urlQuery);

  const submit = () => {
    const trimmed = draft.trim();
    if (!trimmed) return;
    navigate({ pathname: "/search", search: `?q=${encodeURIComponent(trimmed)}` });
  };

  return (
    <form
      role="search"
      className="flex min-w-0 flex-1 max-w-xs items-center gap-2 rounded-md border border-border-subtle bg-bg-surface px-3 py-1.5 transition-colors duration-150 ease-out focus-within:border-accent"
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <Search size={14} aria-hidden="true" className="shrink-0 text-text-tertiary" />
      <input
        key={urlQuery}
        type="search"
        defaultValue={urlQuery}
        onChange={(event) => setDraft(event.target.value)}
        placeholder={t("search.placeholder")}
        aria-label={t("search.inputLabel")}
        className="min-w-0 flex-1 bg-transparent text-sm text-text-primary outline-none placeholder:text-text-tertiary"
      />
    </form>
  );
}

export function TopBar() {
  const location = useLocation();
  const { t } = useTranslation();
  const { preference, setPreference } = useTheme();
  const current = NAV_ITEMS.find((item) => item.path === location.pathname);

  return (
    <header className="flex h-14 shrink-0 items-center justify-between gap-3 border-b border-border-subtle px-5">
      <h1 className="truncate text-md font-semibold text-text-primary">
        {current ? t(`nav.${current.key}`) : t("shell.brand")}
      </h1>

      <HeaderSearch />

      <div className="flex shrink-0 items-center gap-2">
        <LanguageSwitcher />
        <div className="theme-switcher" role="group" aria-label={t("a11y.theme")}>
          {THEME_CHOICES.map((choice) => (
            <button
              key={choice}
              type="button"
              className={cn(
                "rounded-full border-none bg-transparent px-3 py-1 text-sm text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary",
                preference === choice && "bg-accent text-bg-surface hover:bg-accent",
              )}
              aria-pressed={preference === choice}
              onClick={() => setPreference(choice)}
            >
              {t(`theme.${choice}`)}
            </button>
          ))}
        </div>
      </div>
    </header>
  );
}
