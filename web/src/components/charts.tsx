"use client";

import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
  PieChart,
  Pie,
  Cell,
} from "recharts";

type ChartData = {
  date: string;
  message_count: number;
};

const TOOLTIP_STYLE = {
  backgroundColor: "#1e1b28",
  border: "1px solid rgba(255, 255, 255, 0.1)",
  borderRadius: "10px",
  color: "#eeeae2",
  fontSize: "13px",
  padding: "8px 12px",
  boxShadow: "0 8px 32px rgba(0, 0, 0, 0.4)",
};

const TICK_STYLE = { fill: "#eeeae2", opacity: 0.35, fontSize: 11 };

export function MessageActivityChart({ data }: { data: ChartData[] }) {
  if (!data || data.length === 0) {
    return (
      <div className="flex h-[280px] items-center justify-center text-sm text-cream/40">
        No data available
      </div>
    );
  }

  return (
    <div className="h-[280px] w-full">
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart
          data={data}
          margin={{ top: 5, right: 5, left: -10, bottom: 0 }}
        >
          <defs>
            <linearGradient id="colorMessages" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#d4a574" stopOpacity={0.25} />
              <stop offset="100%" stopColor="#d4a574" stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid stroke="rgba(255, 255, 255, 0.04)" vertical={false} />
          <XAxis
            dataKey="date"
            axisLine={false}
            tickLine={false}
            tick={TICK_STYLE}
            dy={10}
            tickFormatter={(val) => val.split("-").slice(1).join("/")}
          />
          <YAxis
            axisLine={false}
            tickLine={false}
            tick={TICK_STYLE}
            dx={-5}
          />
          <Tooltip
            contentStyle={TOOLTIP_STYLE}
            itemStyle={{ color: "#d4a574" }}
            cursor={{ stroke: "rgba(212, 165, 116, 0.2)" }}
          />
          <Area
            type="monotone"
            dataKey="message_count"
            stroke="#d4a574"
            strokeWidth={2}
            fillOpacity={1}
            fill="url(#colorMessages)"
            name="Messages"
            dot={false}
            activeDot={{ r: 4, fill: "#d4a574", stroke: "#1a1724", strokeWidth: 2 }}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

const COLORS = ["#d4a574", "#f0d78c", "#8b7355", "#5a4d57", "#3d3548"];

export function ActivityDistributionChart({
  data,
}: {
  data: { name: string; value: number }[];
}) {
  if (!data || data.length === 0) {
    return (
      <div className="flex h-[280px] items-center justify-center text-sm text-cream/40">
        No data available
      </div>
    );
  }

  return (
    <div className="h-[280px] w-full">
      <ResponsiveContainer width="100%" height="100%">
        <PieChart>
          <Pie
            data={data}
            cx="50%"
            cy="50%"
            innerRadius={65}
            outerRadius={90}
            paddingAngle={3}
            dataKey="value"
            stroke="none"
            cornerRadius={4}
          >
            {data.map((_, index) => (
              <Cell
                key={`cell-${index}`}
                fill={COLORS[index % COLORS.length]}
              />
            ))}
          </Pie>
          <Tooltip
            contentStyle={TOOLTIP_STYLE}
            itemStyle={{ color: "#d4a574" }}
          />
        </PieChart>
      </ResponsiveContainer>
    </div>
  );
}

export function HourlyActivityChart({
  data,
}: {
  data: { hour_label: string; message_count: number }[];
}) {
  if (!data || data.length === 0) {
    return (
      <div className="flex h-[280px] items-center justify-center text-sm text-cream/40">
        No data available
      </div>
    );
  }

  return (
    <div className="h-[280px] w-full">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart
          data={data}
          margin={{ top: 5, right: 5, left: -10, bottom: 0 }}
        >
          <CartesianGrid stroke="rgba(255, 255, 255, 0.04)" vertical={false} />
          <XAxis
            dataKey="hour_label"
            axisLine={false}
            tickLine={false}
            tick={TICK_STYLE}
            interval={3}
            dy={10}
          />
          <YAxis
            axisLine={false}
            tickLine={false}
            tick={TICK_STYLE}
            dx={-5}
          />
          <Tooltip
            contentStyle={TOOLTIP_STYLE}
            itemStyle={{ color: "#d4a574" }}
            cursor={{ fill: "rgba(255, 255, 255, 0.03)" }}
          />
          <Bar
            dataKey="message_count"
            fill="#d4a574"
            radius={[6, 6, 0, 0]}
            name="Messages"
            opacity={0.8}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
