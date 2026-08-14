import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { CheckCircle2, Play, Search } from "lucide-react";
import { media_nodes } from "@/api";
import { queryKeys } from "@/api";
import { Button, Dialog, DialogContent, Skeleton, useToast } from "@/components/ui";
import { useShortcuts } from "@/shortcuts/useShortcuts";
import { formatKeyCombo } from "@/shortcuts/keys";
import { cn } from "@/lib/cn";
import { useMediaSearchQuery } from "@/features/search/api";
import type { MediaListItem } from "./api";
import { ProgressBar } from "./ProgressBar";
import { consumingStateFor } from "./progress";
import { nodeUnitLabel } from "./progress";
import { unreadUnits } from "./progress";
import { useMarkNextUnit } from "./progress";
import { useMarkRange } from "./progress";

/* MISSION-049 — Quick capture. A global popover (Mod+Enter, or the palette
   command) to catch up on progress fast: type-ahead over the library, pick a
   title, then mark the next unit done or the next N units. The palette command
   opens it by dispatching the `mylore:open-quick-capture` window event so the
   palette stays decoupled from this feature.

   The dialog shell owns only the open state and the global shortcut; the
   react-query-driven panel mounts only while open. Keeping queries out of the
   shell lets the shell live under any renderer (e.g. AppShell in the
   preferences tests, which has no QueryClientProvider). */

const AMOUNTS = [1, 5, 10];

function QuickCapturePanel() {
  const { t } = useTranslation();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<MediaListItem | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const activeRef = useRef<HTMLButtonElement>(null);

  const trimmed = query.trim();
  const results = useMediaSearchQuery(trimmed);
  const items = results.data ?? [];

  const nodesQuery = useQuery({
    queryKey: queryKeys.media.nodes(selected?.id ?? ""),
    queryFn: () => media_nodes({ id: selected!.id }),
    enabled: selected !== null,
  });

  const consuming = selected ? consumingStateFor(selected.content_type) : "read";
  const unread = useMemo(
    () => (selected && nodesQuery.data ? unreadUnits(nodesQuery.data, consuming) : []),
    [selected, nodesQuery.data, consuming],
  );

  const markNext = useMarkNextUnit();
  const markRange = useMarkRange(selected?.id ?? "");

  useEffect(() => {
    const frame = requestAnimationFrame(() => inputRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, []);

  const safeActive = Math.min(activeIndex, Math.max(items.length - 1, 0));

  useEffect(() => {
    if (items.length > 0) activeRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [safeActive, items.length]);

  const handleInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => (items.length === 0 ? 0 : (index + 1) % items.length));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) =>
        items.length === 0 ? 0 : (index - 1 + items.length) % items.length,
      );
    } else if (event.key === "Enter") {
      event.preventDefault();
      const item = items[safeActive];
      if (item) setSelected(item);
    } else {
      setActiveIndex(0);
    }
  };

  const markNextUnit = () => {
    if (!selected) return;
    const label =
      unread.length > 0 ? nodeUnitLabel(unread[0]) : (selected.progress.next_label ?? "");
    markNext.mutate(selected.id, {
      onSuccess: (view) => {
        if (!view) {
          toast.info({ title: t("quick.allCaughtUp") });
          return;
        }
        if (label) toast.success({ title: t("quick.doneToast", { label }) });
      },
    });
  };

  const markUpTo = (count: number) => {
    if (!selected || unread.length === 0) return;
    const to = unread[Math.min(count, unread.length) - 1];
    markRange.mutate(
      { fromId: unread[0].id, toId: to.id, state: consuming },
      {
        onSuccess: () => toast.success({ title: t("quick.rangeDone", { count }) }),
      },
    );
  };

  const amounts = useMemo(
    () => Array.from(new Set([...AMOUNTS.filter((a) => a < unread.length), unread.length])),
    [unread.length],
  );

  const progress = selected?.progress ?? null;

  return (
    <div className="flex max-h-[70vh]">
      <div className="flex w-72 shrink-0 flex-col border-e border-border-subtle">
        <div className="flex items-center gap-3 border-b border-border-subtle px-4">
          <Search size={16} className="shrink-0 text-text-tertiary" aria-hidden="true" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setActiveIndex(0);
            }}
            onKeyDown={handleInputKeyDown}
            placeholder={t("quick.placeholder")}
            aria-label={t("quick.placeholder")}
            role="combobox"
            aria-expanded
            aria-controls="quick-capture-results"
            className="h-12 w-full border-none bg-transparent text-base text-text-primary outline-none placeholder:text-text-tertiary"
          />
          <kbd className="hidden shrink-0 rounded-sm border border-border-subtle bg-bg-base px-1.5 py-0.5 text-xs text-text-tertiary sm:inline-block">
            {formatKeyCombo("Mod+Enter")}
          </kbd>
        </div>

        <div
          id="quick-capture-results"
          role="listbox"
          aria-label={t("quick.title")}
          className="min-h-40 flex-1 overflow-y-auto p-2"
        >
          {!trimmed ? (
            <p className="px-3 py-6 text-center text-sm text-text-tertiary">
              {t("quick.pickHint")}
            </p>
          ) : results.isPending ? (
            <div className="flex flex-col gap-2 p-2">
              <Skeleton className="h-9 w-full" />
              <Skeleton className="h-9 w-full" />
            </div>
          ) : items.length === 0 ? (
            <p className="px-3 py-6 text-center text-sm text-text-secondary">
              {t("quick.searchEmpty")}
            </p>
          ) : (
            items.map((item, index) => {
              const active = index === safeActive;
              const isSelected = selected?.id === item.id;
              return (
                <button
                  key={item.id}
                  ref={active ? activeRef : undefined}
                  type="button"
                  role="option"
                  aria-selected={active}
                  onMouseEnter={() => setActiveIndex(index)}
                  onClick={() => setSelected(item)}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-sm px-3 py-2 text-start text-sm",
                    active ? "bg-accent-soft text-accent" : "text-text-primary hover:bg-bg-hover",
                    isSelected && "ring-1 ring-inset ring-accent",
                  )}
                >
                  <span className="flex-1 truncate">{item.title}</span>
                  {item.progress.next_label ? (
                    <span className="shrink-0 text-xs tabular-nums text-text-tertiary">
                      {item.progress.next_label}
                    </span>
                  ) : null}
                </button>
              );
            })
          )}
        </div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col p-4">
        {!selected ? (
          <p className="py-8 text-center text-sm text-text-tertiary">{t("quick.pickHint")}</p>
        ) : (
          <>
            <div className="flex items-center gap-2">
              <h2 className="min-w-0 flex-1 truncate text-base font-medium text-text-primary">
                {selected.title}
              </h2>
              <span className="shrink-0 rounded-full border border-border-subtle px-2 py-0.5 text-xs text-text-tertiary">
                {t(`contentType.${selected.content_type}`)}
              </span>
            </div>

            {progress ? (
              <div className="mt-3 flex items-center gap-2">
                <ProgressBar percent={progress.percent} className="flex-1" />
                <span className="shrink-0 text-xs tabular-nums text-text-tertiary">
                  {progress.completed}/{progress.total}
                </span>
              </div>
            ) : null}

            <div className="mt-5">
              {nodesQuery.isPending ? (
                <div className="flex flex-col gap-2">
                  <Skeleton className="h-10 w-full" />
                  <Skeleton className="h-10 w-full" />
                </div>
              ) : nodesQuery.isError ? (
                <div className="flex flex-col items-start gap-3">
                  <p className="text-sm text-text-secondary">{t("quick.errorTitle")}</p>
                  <Button size="sm" onClick={() => nodesQuery.refetch()}>
                    {t("tracking.retry")}
                  </Button>
                </div>
              ) : unread.length === 0 ? (
                <div className="flex flex-col items-center gap-2 py-6 text-center">
                  <CheckCircle2 size={24} className="text-accent" aria-hidden="true" />
                  <p className="text-sm font-medium text-text-primary">{t("quick.allCaughtUp")}</p>
                  <p className="text-xs text-text-tertiary">{t("quick.allCaughtUpHint")}</p>
                </div>
              ) : (
                <div className="flex flex-col gap-4">
                  <button
                    type="button"
                    onClick={markNextUnit}
                    disabled={markNext.isPending}
                    className="inline-flex items-center justify-center gap-2 rounded-md border border-accent bg-accent-soft px-4 py-2 text-sm font-medium text-accent transition-colors duration-150 ease-out hover:bg-accent hover:text-bg-surface disabled:opacity-60"
                  >
                    <Play size={14} aria-hidden="true" />
                    {t("quick.nextAction")}
                    <span className="tabular-nums">{nodeUnitLabel(unread[0])}</span>
                  </button>

                  <div className="flex flex-col gap-2">
                    <span className="text-xs font-medium uppercase tracking-wide text-text-tertiary">
                      {t("quick.rangeLabel")}
                    </span>
                    <div className="flex flex-wrap gap-2">
                      {amounts.map((count) => (
                        <button
                          key={count}
                          type="button"
                          onClick={() => markUpTo(count)}
                          disabled={markRange.isPending || markNext.isPending}
                          aria-label={t("quick.rangeUpTo", { count })}
                          className="inline-flex h-9 min-w-12 items-center justify-center rounded-md border border-border-subtle px-3 text-sm text-text-primary transition-colors duration-150 ease-out hover:border-accent hover:text-accent disabled:opacity-60"
                        >
                          {count === unread.length && unread.length > 10
                            ? t("quick.rangeAll")
                            : count}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export function QuickCapture() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  const toggle = useCallback(() => setOpen((value) => !value), []);
  useShortcuts([{ combo: "Mod+Enter", handler: toggle }]);

  useEffect(() => {
    const handler = () => setOpen(true);
    window.addEventListener("mylore:open-quick-capture", handler);
    return () => window.removeEventListener("mylore:open-quick-capture", handler);
  }, []);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        className="max-w-2xl"
        noPadding
        closeLabel={t("a11y.close")}
        onOpenAutoFocus={(event) => event.preventDefault()}
      >
        {open ? <QuickCapturePanel /> : null}
      </DialogContent>
    </Dialog>
  );
}
