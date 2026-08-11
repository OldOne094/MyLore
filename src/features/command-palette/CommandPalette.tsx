import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import { Search } from "lucide-react";
import { Dialog, DialogContent } from "@/components/ui";
import { usePreferences } from "@/preferences/usePreferences";
import { useShortcuts } from "@/shortcuts/useShortcuts";
import { formatKeyCombo } from "@/shortcuts/keys";
import { cn } from "@/lib/cn";
import {
  buildPaletteCommands,
  filterPaletteCommands,
  PALETTE_GROUPS,
  type PaletteCommand,
} from "./commands";

/* MISSION-036 — Command palette skeleton. Opens with Ctrl/Cmd+K, filters
   commands by label/keywords, navigates with ↑/↓ + Enter, runs on click.
   Full action map ships with MISSION-089. */

export function CommandPalette() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { setTheme } = usePreferences();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const activeRef = useRef<HTMLButtonElement>(null);

  const toggle = useCallback(() => setOpen((value) => !value), []);
  useShortcuts([{ combo: "Mod+K", handler: toggle }]);

  const handleOpenChange = useCallback((next: boolean) => {
    setOpen(next);
    if (next) {
      setQuery("");
      setActiveIndex(0);
    }
  }, []);

  const commands = useMemo(
    () => buildPaletteCommands({ t, navigate, setTheme }),
    [t, navigate, setTheme],
  );
  const filtered = useMemo(() => filterPaletteCommands(commands, query), [commands, query]);

  // Focus the query input once the dialog is open (no state writes here).
  useEffect(() => {
    if (!open) return;
    const frame = requestAnimationFrame(() => inputRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, [open]);

  const safeActive = Math.min(activeIndex, Math.max(filtered.length - 1, 0));

  useEffect(() => {
    if (open) activeRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [safeActive, open]);

  const runCommand = useCallback((command: PaletteCommand) => {
    setOpen(false);
    command.run();
  }, []);

  const handleInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => (filtered.length === 0 ? 0 : (index + 1) % filtered.length));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) =>
        filtered.length === 0 ? 0 : (index - 1 + filtered.length) % filtered.length,
      );
    } else if (event.key === "Enter") {
      event.preventDefault();
      const command = filtered[safeActive];
      if (command) runCommand(command);
    } else {
      setActiveIndex(0);
    }
  };

  let optionIndex = 0;
  const sections = PALETTE_GROUPS.map((group) => ({
    group,
    items: filtered.filter((command) => command.group === group),
  })).filter((section) => section.items.length > 0);

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent
        className="max-w-xl"
        noPadding
        onOpenAutoFocus={(event) => event.preventDefault()}
      >
        <div className="flex items-center gap-3 border-b border-border-subtle px-4">
          <Search size={16} className="shrink-0 text-text-tertiary" aria-hidden="true" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={handleInputKeyDown}
            placeholder={t("palette.placeholder")}
            aria-label={t("palette.placeholder")}
            role="combobox"
            aria-expanded={open}
            aria-controls="command-palette-list"
            className="h-12 w-full border-none bg-transparent text-base text-text-primary outline-none placeholder:text-text-tertiary"
          />
          <kbd className="hidden shrink-0 rounded-sm border border-border-subtle bg-bg-base px-1.5 py-0.5 text-xs text-text-tertiary sm:inline-block">
            {formatKeyCombo("Mod+K")}
          </kbd>
        </div>

        <div
          id="command-palette-list"
          role="listbox"
          aria-label={t("palette.title")}
          className="max-h-80 overflow-y-auto p-2"
        >
          {sections.length === 0 ? (
            <p className="px-3 py-6 text-center text-sm text-text-secondary">
              {t("palette.empty")}
            </p>
          ) : (
            sections.map(({ group, items }) => (
              <div key={group}>
                <p className="px-3 pb-1 pt-2 text-xs font-semibold uppercase tracking-wide text-text-tertiary">
                  {t(`palette.group_${group}`)}
                </p>
                {items.map((command) => {
                  const index = optionIndex++;
                  const active = index === safeActive;
                  return (
                    <button
                      key={command.id}
                      ref={active ? activeRef : undefined}
                      type="button"
                      role="option"
                      aria-selected={active}
                      onMouseEnter={() => setActiveIndex(index)}
                      onClick={() => runCommand(command)}
                      className={cn(
                        "flex w-full items-center gap-3 rounded-sm px-3 py-2 text-start text-sm",
                        active
                          ? "bg-accent-soft text-accent"
                          : "text-text-primary hover:bg-bg-hover",
                      )}
                    >
                      <command.icon
                        size={16}
                        className="shrink-0 text-text-tertiary"
                        aria-hidden="true"
                      />
                      <span className="flex-1 truncate">{command.label}</span>
                      {command.hint ? (
                        <kbd className="shrink-0 rounded-sm border border-border-subtle bg-bg-base px-1.5 py-0.5 text-xs text-text-tertiary">
                          {command.hint}
                        </kbd>
                      ) : null}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
