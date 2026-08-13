import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  CircleDot,
  CircleSlash,
  ListTree,
  RefreshCcw,
} from "lucide-react";
import { Badge, Button, EmptyState, Skeleton } from "@/components/ui";
import type { ContentNode } from "@/api";
import { useMediaNodesQuery, useNodeProgress } from "./api";

/* MISSION-046/MISSION-047 — Content tree for the media detail page (Details
   tab). Renders the nested season→episode / volume→chapter tree with
   expand/collapse and per-node progress toggling.
   WAI-ARIA tree semantics: `role=tree`/`treeitem` with aria-level/setsize/
   posinset/expanded, roving tabindex and Arrow Up/Down (move) + Arrow
   Right/Left (expand/collapse) keyboard nav. Shift-click (or Shift+Enter on
   the row) marks every node between the last toggled row and the target. */

interface FlatRow {
  node: ContentNode;
  depth: number;
  expanded: boolean;
  posinset: number;
  setsize: number;
}

const EMPTY_NODES: ContentNode[] = [];
const COMPLETED = new Set(["read", "watched"]);

/** Pre-order traversal of the tree — the canonical display order used to slice
    shift-click ranges regardless of which rows are currently expanded. */
function preorderIds(nodes: ContentNode[]): string[] {
  const ids: string[] = [];
  const walk = (items: ContentNode[]) => {
    for (const node of items) {
      ids.push(node.id);
      walk(node.children);
    }
  };
  walk(nodes);
  return ids;
}

function isCompleted(node: ContentNode): boolean {
  return node.state != null && COMPLETED.has(node.state);
}

function stateIcon(node: ContentNode) {
  if (isCompleted(node)) return <CheckCircle2 size={16} className="text-accent" />;
  if (node.state === "partial") return <CircleDot size={16} className="text-text-tertiary" />;
  return <Circle size={16} className="text-text-tertiary" />;
}

function consumingState(contentType: string): "read" | "watched" {
  return ["anime", "tv", "movie"].includes(contentType) ? "watched" : "read";
}

function flattenVisible(nodes: ContentNode[], expanded: Set<string>, depth = 0): FlatRow[] {
  const rows: FlatRow[] = [];
  nodes.forEach((node, index) => {
    const open = node.children.length > 0 && expanded.has(node.id);
    rows.push({ node, depth, expanded: open, posinset: index + 1, setsize: nodes.length });
    if (open) rows.push(...flattenVisible(node.children, expanded, depth + 1));
  });
  return rows;
}

function nodeLabel(node: ContentNode, kindLabel: string): string {
  if (node.number && node.title) return `${kindLabel} ${node.number} · ${node.title}`;
  if (node.number) return `${kindLabel} ${node.number}`;
  if (node.title) return node.title;
  return kindLabel;
}

export function NodeTree({
  mediaId,
  mediaTitle,
  contentType,
}: {
  mediaId: string;
  mediaTitle: string;
  contentType: string;
}) {
  const { t } = useTranslation();
  const { data, isPending, isError, refetch } = useMediaNodesQuery(mediaId);
  const { markNode, markRange } = useNodeProgress(mediaId);
  const nodes = data ?? EMPTY_NODES;
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [focusId, setFocusId] = useState<string | null>(null);
  const [anchorId, setAnchorId] = useState<string | null>(null);
  const seeded = useRef(false);
  const itemRefs = useRef(new Map<string, HTMLElement | null>());

  const order = useMemo(() => preorderIds(nodes), [nodes]);
  const complete = consumingState(contentType);

  useEffect(() => {
    if (seeded.current || nodes.length === 0) return;
    seeded.current = true;
    setExpanded(new Set(nodes.filter((node) => node.children.length > 0).map((node) => node.id)));
  }, [nodes]);

  const rows = flattenVisible(nodes, expanded);
  const focusedVisible = focusId !== null && rows.some((row) => row.node.id === focusId);

  useEffect(() => {
    if (focusId) itemRefs.current.get(focusId)?.focus();
  }, [focusId, expanded]);

  if (isPending) {
    return (
      <div role="status" aria-label={t("nodes.loading")} className="flex flex-col gap-2">
        {[0, 1, 2, 3, 4].map((index) => (
          <Skeleton key={index} className="h-8 w-full" />
        ))}
      </div>
    );
  }

  if (isError) {
    return (
      <EmptyState
        icon={RefreshCcw}
        title={t("nodes.errorTitle")}
        hint={t("nodes.errorHint")}
        action={<Button onClick={() => refetch()}>{t("nodes.retry")}</Button>}
      />
    );
  }

  if (nodes.length === 0) {
    return <EmptyState icon={ListTree} title={t("nodes.emptyTitle")} hint={t("nodes.emptyHint")} />;
  }

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  /** Toggle a single node's progress, or — with shift held — the whole range
      from the last toggled row to the target. The next state depends on the
      target: completed becomes unread, anything else becomes completed. */
  const activate = (node: ContentNode, shift: boolean) => {
    setFocusId(node.id);
    const next = isCompleted(node) ? "unread" : complete;
    if (!shift || anchorId === null || anchorId === node.id) {
      setAnchorId(node.id);
      void markNode(node.id, next);
      return;
    }
    const from = order.indexOf(anchorId);
    const to = order.indexOf(node.id);
    if (from === -1 || to === -1) {
      setAnchorId(node.id);
      void markNode(node.id, next);
      return;
    }
    const rangeIds = order.slice(Math.min(from, to), Math.max(from, to) + 1);
    setAnchorId(node.id);
    void markRange(anchorId, node.id, next, rangeIds);
  };

  /** Toggle a node's skipped state (skip ↔ unread). */
  const skip = (node: ContentNode) => {
    setFocusId(node.id);
    void markNode(node.id, node.state === "skipped" ? "unread" : "skipped");
  };

  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    const index = rows.findIndex((row) => row.node.id === focusId);
    if (index === -1) return;
    const row = rows[index];
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setFocusId(rows[Math.min(index + 1, rows.length - 1)].node.id);
        break;
      case "ArrowUp":
        event.preventDefault();
        setFocusId(rows[Math.max(index - 1, 0)].node.id);
        break;
      case "ArrowRight":
        event.preventDefault();
        if (row.node.children.length > 0 && !row.expanded) {
          setExpanded((prev) => new Set(prev).add(row.node.id));
        }
        break;
      case "ArrowLeft":
        event.preventDefault();
        if (row.node.children.length > 0 && row.expanded) {
          setExpanded((prev) => {
            const next = new Set(prev);
            next.delete(row.node.id);
            return next;
          });
        }
        break;
    }
  };

  return (
    <>
      <p className="pb-1 text-xs text-text-tertiary">{t("progress.rangeHint")}</p>
      <ul
        role="tree"
        aria-label={t("nodes.treeLabel", { title: mediaTitle })}
        onKeyDown={onKeyDown}
        className="flex flex-col"
      >
        {rows.map((row) => {
          const { node } = row;
          const kindLabel = t(`nodeKind.${node.kind}`, { defaultValue: t("nodeKind.node") });
          const label = nodeLabel(node, kindLabel);
          const hasChildren = node.children.length > 0;
          const isChecked = isCompleted(node);
          const isSkipped = node.state === "skipped";
          return (
            <li
              key={node.id}
              ref={(element) => {
                itemRefs.current.set(node.id, element);
              }}
              role="treeitem"
              aria-level={row.depth + 1}
              aria-posinset={row.posinset}
              aria-setsize={row.setsize}
              aria-expanded={hasChildren ? row.expanded : undefined}
              aria-label={label}
              tabIndex={
                focusedVisible ? (row.node.id === focusId ? 0 : -1) : row === rows[0] ? 0 : -1
              }
              onFocus={() => setFocusId(node.id)}
              onClick={(event) => {
                if (event.target === event.currentTarget) activate(node, event.shiftKey);
              }}
              onKeyDown={(event) => {
                if (event.target !== event.currentTarget) return;
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  activate(node, event.shiftKey);
                }
              }}
              className="group flex cursor-pointer items-center gap-2 rounded-md py-1.5 pr-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
              style={{ paddingInlineStart: 8 + row.depth * 16 }}
            >
              {hasChildren ? (
                <button
                  type="button"
                  tabIndex={-1}
                  aria-label={
                    row.expanded ? t("nodes.collapse", { label }) : t("nodes.expand", { label })
                  }
                  onClick={() => {
                    setFocusId(node.id);
                    toggle(node.id);
                  }}
                  className="flex h-6 w-6 shrink-0 items-center justify-center rounded text-text-tertiary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary"
                >
                  {row.expanded ? (
                    <ChevronDown size={16} aria-hidden="true" />
                  ) : (
                    <ChevronRight size={16} aria-hidden="true" className="rtl:rotate-180" />
                  )}
                </button>
              ) : (
                <span
                  aria-hidden="true"
                  className="flex h-6 w-6 shrink-0 items-center justify-center"
                >
                  <span className="w-4" />
                </span>
              )}

              <button
                type="button"
                role="checkbox"
                aria-checked={node.state === "partial" ? "mixed" : isChecked}
                aria-label={
                  isChecked
                    ? t("progress.unmark", { label })
                    : complete === "watched"
                      ? t("progress.toggleWatched", { label })
                      : t("progress.toggleRead", { label })
                }
                tabIndex={-1}
                onClick={(event) => {
                  event.stopPropagation();
                  activate(node, event.shiftKey);
                }}
                className="flex h-6 w-6 shrink-0 items-center justify-center rounded text-text-tertiary transition-colors duration-150 ease-out hover:text-accent"
              >
                {stateIcon(node)}
              </button>

              <span className="min-w-0 truncate text-sm text-text-primary">{label}</span>

              {node.is_special ? <Badge variant="accent">{t("nodes.special")}</Badge> : null}

              {node.duration_min != null ? (
                <span className="shrink-0 text-xs tabular-nums text-text-tertiary">
                  {t("nodes.duration", { count: node.duration_min })}
                </span>
              ) : null}

              {node.page_count != null ? (
                <span className="shrink-0 text-xs tabular-nums text-text-tertiary">
                  {t("nodes.pages", { count: node.page_count })}
                </span>
              ) : null}

              <span className="flex-1" />

              <button
                type="button"
                tabIndex={-1}
                aria-label={
                  isSkipped ? t("progress.unskip", { label }) : t("progress.skip", { label })
                }
                onClick={(event) => {
                  event.stopPropagation();
                  skip(node);
                }}
                className="flex h-6 w-6 shrink-0 items-center justify-center rounded text-text-tertiary opacity-0 transition-opacity duration-150 ease-out hover:bg-bg-hover hover:text-accent focus-visible:opacity-100 group-hover:opacity-100"
              >
                <CircleSlash
                  size={14}
                  aria-hidden="true"
                  className={isSkipped ? "text-accent opacity-100" : undefined}
                />
              </button>
            </li>
          );
        })}
      </ul>
    </>
  );
}
