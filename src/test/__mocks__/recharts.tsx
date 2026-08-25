import { createElement } from "react";

// MISSION-113 — Stub replacement for recharts in test environments.
// Recharts requires real DOM layout (ResizeObserver + getBBox) which jsdom
// doesn't provide. This module exports no-op components so chart wrappers
// mount without crashing and tests assert on non-chart DOM elements.

function NullComponent() {
  return null;
}

function Container(props: Record<string, unknown> & { children?: unknown }) {
  return createElement("div", null, props.children as React.ReactNode);
}

export const ResponsiveContainer = Container;
export const AreaChart = Container;
export const BarChart = Container;
export const PieChart = Container;
export const Area = NullComponent;
export const Bar = NullComponent;
export function Pie(props: Record<string, unknown> & { children?: unknown }) {
  return createElement("div", null, props.children as React.ReactNode);
}
export const Cell = NullComponent;
export const CartesianGrid = NullComponent;
export const XAxis = NullComponent;
export const YAxis = NullComponent;
export const Tooltip = NullComponent;
