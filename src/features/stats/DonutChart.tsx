import { Cell, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";
import { ChartBoundary } from "./ChartBoundary";
import { cssVar, useChartTheme } from "./chartTheme";

/* MISSION-113 — Donut chart for categorical distributions with a text legend
   for accessibility and test visibility. */

const DONUT_COLORS = ["var(--accent)", "var(--info)", "var(--ok)", "var(--warn)", "var(--danger)"];

export function DonutChart({
  data,
  height = 200,
}: {
  data: { key: string; label: string; count: number }[];
  height?: number;
}) {
  const theme = useChartTheme();
  const visible = data.filter((d) => d.count > 0);
  if (visible.length === 0) return null;

  return (
    <ChartBoundary>
      <ResponsiveContainer width="100%" height={height}>
        <PieChart>
          <Tooltip contentStyle={theme.tooltip} />
          <Pie
            data={visible}
            dataKey="count"
            nameKey="label"
            cx="50%"
            cy="50%"
            innerRadius={height * 0.25}
            outerRadius={height * 0.4}
            paddingAngle={2}
            strokeWidth={2}
            stroke={cssVar("--bg-surface")}
          >
            {visible.map((_, index) => (
              <Cell
                key={`cell-${index}`}
                fill={
                  index < DONUT_COLORS.length
                    ? cssVar(DONUT_COLORS[index])
                    : cssVar("--text-tertiary")
                }
              />
            ))}
          </Pie>
        </PieChart>
      </ResponsiveContainer>
      {/* Accessible text legend */}
      <ul className="mt-2 flex flex-col gap-1">
        {visible.map((row) => (
          <li key={row.key} className="flex items-center justify-between gap-2">
            <span className="truncate text-xs text-text-secondary">{row.label}</span>
            <span className="shrink-0 text-xs tabular-nums text-text-tertiary">{row.count}</span>
          </li>
        ))}
      </ul>
    </ChartBoundary>
  );
}
