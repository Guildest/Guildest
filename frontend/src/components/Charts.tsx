"use client";

import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

export function SimpleLineChart(props: {
  data: Array<Record<string, unknown>>;
  xKey: string;
  yKey: string;
}) {
  return (
    <div className="h-64 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={props.data}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey={props.xKey} hide />
          <YAxis width={32} />
          <Tooltip />
          <Line type="monotone" dataKey={props.yKey} stroke="hsl(var(--primary))" dot={false} />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}

