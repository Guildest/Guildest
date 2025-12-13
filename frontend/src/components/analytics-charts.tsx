"use client";

import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  BarChart,
  Bar,
} from "recharts";
import { MessageCountPoint, SentimentPoint } from "@/lib/types";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatDate } from "@/lib/utils";

interface MessageVolumeChartProps {
  data: MessageCountPoint[];
}

export function MessageVolumeChart({ data }: MessageVolumeChartProps) {
  // Format data for chart
  const chartData = data.map((d) => ({
    date: new Date(d.time_bucket).toLocaleDateString(),
    count: d.count,
  }));

  return (
    <Card className="col-span-4">
      <CardHeader>
        <CardTitle>Message Volume</CardTitle>
      </CardHeader>
      <CardContent className="pl-2">
        <ResponsiveContainer width="100%" height={350}>
          <BarChart data={chartData}>
            <XAxis
              dataKey="date"
              stroke="#626C66"
              fontSize={12}
              tickLine={false}
              axisLine={false}
            />
            <YAxis
              stroke="#626C66"
              fontSize={12}
              tickLine={false}
              axisLine={false}
              tickFormatter={(value) => `${value}`}
            />
            <Tooltip 
                contentStyle={{ backgroundColor: 'hsl(var(--card))', borderColor: 'hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                itemStyle={{ color: 'hsl(var(--foreground))' }}
                cursor={{ fill: 'hsl(var(--muted))' }}
            />
            <Bar dataKey="count" fill="currentColor" radius={[4, 4, 0, 0]} className="fill-primary" />
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

interface SentimentChartProps {
  data: SentimentPoint[];
}

export function SentimentChart({ data }: SentimentChartProps) {
    const chartData = data.map(d => ({
        date: formatDate(d.day),
        score: d.score !== null ? d.score : 0
    }));

  return (
    <Card className="col-span-4">
      <CardHeader>
        <CardTitle>Sentiment Trends</CardTitle>
      </CardHeader>
      <CardContent className="pl-2">
        <ResponsiveContainer width="100%" height={350}>
          <LineChart data={chartData}>
             <XAxis
              dataKey="date"
              stroke="#888888"
              fontSize={12}
              tickLine={false}
              axisLine={false}
            />
            <YAxis
              stroke="#888888"
              fontSize={12}
              tickLine={false}
              axisLine={false}
              domain={[-1, 1]} 
            />
            <Tooltip 
                 contentStyle={{ backgroundColor: 'var(--background)', borderColor: 'var(--border)' }}
                 itemStyle={{ color: 'var(--foreground)' }}
            />
            <Line
              type="monotone"
              dataKey="score"
              stroke="hsl(var(--primary))"
              strokeWidth={2}
              activeDot={{ r: 8 }}
            />
          </LineChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}
