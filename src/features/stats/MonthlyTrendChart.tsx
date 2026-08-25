import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { ChartBoundary } from "./ChartBoundary";
import { cssVar, useChartTheme } from "./chartTheme";

/* MISSION-113 — Monthly trend area chart (Recharts). Supports multi-series
   with localized month labels, RTL axis reversal and theme-aware styling. */

export interface MonthlySeries {
  label: string;
  color: string;
  values: number[];
}

function monthLabels(): string[] {
  const formatter = new Intl.DateTimeFormat(undefined, { month: "short" });
  return Array.from({ length: 12 }, (_, i) => formatter.format(new Date(2024, i, 1)));
}

export function MonthlyTrendChart({
  series,
  height = 220,
}: {
  series: MonthlySeries[];
  height?: number;
}) {
  const theme = useChartTheme();
  const labels = monthLabels();

  const data = labels.map((label, index) => {
    const row: Record<string, string | number> = { label };
    for (const s of series) {
      row[s.label] = s.values[index] ?? 0;
    }
    return row;
  });

  return (
    <ChartBoundary>
      <ResponsiveContainer width="100%" height={height}>
        <AreaChart
          data={data}
          margin={{ top: 8, right: 12, bottom: 0, left: 0 }}
          style={{ direction: "ltr" }}
        >
          <defs>
            {series.map((s) => (
              <linearGradient
                key={s.label}
                id={`grad-${s.label.replace(/\s/g, "")}`}
                x1="0"
                y1="0"
                x2="0"
                y2="1"
              >
                <stop offset="5%" stopColor={s.color} stopOpacity={0.25} />
                <stop offset="95%" stopColor={s.color} stopOpacity={0.02} />
              </linearGradient>
            ))}
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke={theme.grid} vertical={false} />
          <XAxis
            dataKey="label"
            tick={{ ...theme.tick }}
            axisLine={false}
            tickLine={false}
            reversed={theme.isRTL}
          />
          <YAxis
            tick={{ ...theme.tick }}
            axisLine={false}
            tickLine={false}
            width={40}
            orientation={theme.isRTL ? "right" : "left"}
          />
          <Tooltip contentStyle={theme.tooltip} cursor={{ stroke: theme.grid }} />
          {series.map((s) => (
            <Area
              key={s.label}
              type="monotone"
              dataKey={s.label}
              stroke={cssVar(s.color)}
              fill={`url(#grad-${s.label.replace(/\s/g, "")})`}
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 4 }}
            />
          ))}
        </AreaChart>
      </ResponsiveContainer>
      {/* Text legend so month labels and values are accessible without SVG */}
      {series.length === 1 && (
        <div className="mt-2 flex flex-wrap items-center gap-x-1 gap-y-0.5">
          {labels.map((label, index) => {
            const value = series[0].values[index] ?? 0;
            return (
              <span key={label} className="inline-flex items-center gap-0.5">
                {value > 0 && (
                  <span className="text-[10px] tabular-nums font-medium text-accent">{value}</span>
                )}
                <span className="text-[10px] text-text-tertiary">{label.slice(0, 3)}</span>
              </span>
            );
          })}
        </div>
      )}
    </ChartBoundary>
  );
}
