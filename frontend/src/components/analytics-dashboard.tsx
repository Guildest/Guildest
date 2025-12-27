"use client";

import { useEffect, useMemo, useState, type ElementType } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  Activity,
  ArrowDownRight,
  ArrowUpRight,
  BarChart3,
  CalendarRange,
  LineChart as LineChartIcon,
  RefreshCw,
  Users2,
  Volume2,
} from "lucide-react";
import {
  AnalyticsCommandPoint,
  AnalyticsMessageBucketPoint,
  AnalyticsSummaryPoint,
  AnalyticsTopChannel,
  AnalyticsTopCommand,
  AnalyticsVoicePoint,
  MessageCountPoint,
  SentimentPoint,
} from "@/lib/types";
import { cn, formatDate } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { SentimentChart } from "@/components/analytics-charts";

type RangePreset = "24h" | "7d" | "30d" | "custom";
type Granularity = "auto" | "hour" | "day";
type MetricMode = "messages" | "voice" | "both";
type AnalyticsTab = "messages" | "voice" | "sentiment" | "commands";

const HOUR_MS = 60 * 60 * 1000;
const DAY_MS = 24 * HOUR_MS;

const RANGE_PRESETS: { value: RangePreset; label: string; hours: number }[] = [
  { value: "24h", label: "24h", hours: 24 },
  { value: "7d", label: "7d", hours: 24 * 7 },
  { value: "30d", label: "30d", hours: 24 * 30 },
  { value: "custom", label: "Custom", hours: 0 },
];

function parseIso(value?: string | null) {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return date;
}

function formatCompact(value: number) {
  return new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function formatPercent(value: number) {
  const formatted = new Intl.NumberFormat("en-US", { maximumFractionDigits: 1 }).format(Math.abs(value));
  return `${formatted}%`;
}

function clampToRange(date: Date, start?: Date | null, end?: Date | null) {
  if (!start || !end) return true;
  return date >= start && date <= end;
}

type SeriesPoint = { ts: number; value: number };

function aggregateSeries<T>(
  points: T[],
  bucketMs: number,
  start: Date | null,
  end: Date | null,
  getTimestamp: (point: T) => number | null,
  getValue: (point: T) => number,
) {
  const map = new Map<number, number>();
  points.forEach((point) => {
    const ts = getTimestamp(point);
    if (!ts) return;
    const date = new Date(ts);
    if (!clampToRange(date, start, end)) return;
    const bucket = Math.floor(ts / bucketMs) * bucketMs;
    map.set(bucket, (map.get(bucket) ?? 0) + getValue(point));
  });
  return Array.from(map.entries())
    .sort((a, b) => a[0] - b[0])
    .map(([ts, value]) => ({ ts, value }));
}

function sumSeries(series: SeriesPoint[]) {
  return series.reduce((acc, point) => acc + point.value, 0);
}

function maxSeries(series: SeriesPoint[]) {
  return series.reduce((acc, point) => Math.max(acc, point.value), 0);
}

function deltaPercent(current: number, previous: number) {
  if (previous === 0) return null;
  return ((current - previous) / previous) * 100;
}

function formatAxisLabel(ts: number, bucketMs: number) {
  const date = new Date(ts);
  if (bucketMs <= HOUR_MS) {
    return date.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
  }
  return date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

type KpiCardProps = {
  title: string;
  value: number | null;
  icon: ElementType;
  delta?: number | null;
  series?: SeriesPoint[];
  footnote?: string;
};

function KpiCard({ title, value, icon: Icon, delta, series, footnote }: KpiCardProps) {
  const trend = delta === null || delta === undefined ? null : delta >= 0 ? "up" : "down";
  return (
    <Card className="rounded-2xl border bg-card/80 p-4">
      <div className="flex items-center justify-between">
        <div className="text-sm text-muted-foreground">{title}</div>
        <span className="rounded-full border bg-background/70 p-2 text-muted-foreground">
          <Icon className="h-4 w-4" />
        </span>
      </div>
      <div className="mt-3 flex items-end justify-between gap-3">
        <div>
          <div className="text-2xl font-semibold">{value === null ? "—" : formatCompact(value)}</div>
          {trend ? (
            <div
              className={cn(
                "mt-1 inline-flex items-center gap-1 text-xs",
                trend === "up" ? "text-emerald-400" : "text-rose-400",
              )}
            >
              {trend === "up" ? <ArrowUpRight className="h-3 w-3" /> : <ArrowDownRight className="h-3 w-3" />}
              {formatPercent(delta ?? 0)}
              <span className="text-muted-foreground">vs prior</span>
            </div>
          ) : (
            <div className="mt-1 text-xs text-muted-foreground">No comparison yet</div>
          )}
        </div>
        {series && series.length > 1 ? (
          <div className="h-12 w-24">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={series}>
                <Line type="monotone" dataKey="value" stroke="hsl(var(--primary))" strokeWidth={2} dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>
        ) : (
          <div className="text-xs text-muted-foreground">{footnote ?? "No data yet"}</div>
        )}
      </div>
    </Card>
  );
}

type AnalyticsDashboardProps = {
  guildId: string;
  messageCounts: MessageCountPoint[];
  sentimentPoints: SentimentPoint[];
  summaryPoints: AnalyticsSummaryPoint[];
  voicePoints: AnalyticsVoicePoint[];
  commandPoints: AnalyticsCommandPoint[];
  topChannels: AnalyticsTopChannel[];
  topCommands: AnalyticsTopCommand[];
};

export function AnalyticsDashboard({
  guildId,
  messageCounts,
  sentimentPoints,
  summaryPoints,
  voicePoints,
  commandPoints,
  topChannels,
  topCommands,
}: AnalyticsDashboardProps) {
  const [rangePreset, setRangePreset] = useState<RangePreset>("7d");
  const [granularity, setGranularity] = useState<Granularity>("auto");
  const [metricMode, setMetricMode] = useState<MetricMode>("messages");
  const [tab, setTab] = useState<AnalyticsTab>("messages");
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");
  const [channelFilter, setChannelFilter] = useState("all");
  const [channelBucketPoints, setChannelBucketPoints] = useState<AnalyticsMessageBucketPoint[]>([]);
  const [channelLoading, setChannelLoading] = useState(false);

  const now = useMemo(() => new Date(), []);

  const range = useMemo(() => {
    if (rangePreset !== "custom") {
      const hours = RANGE_PRESETS.find((preset) => preset.value === rangePreset)?.hours ?? 24;
      return { start: new Date(Date.now() - hours * HOUR_MS), end: new Date() };
    }
    const start = customStart ? parseIso(`${customStart}T00:00:00`) : null;
    const end = customEnd ? parseIso(`${customEnd}T23:59:59`) : null;
    return { start, end };
  }, [customEnd, customStart, rangePreset]);

  useEffect(() => {
    if (channelFilter === "all") {
      setChannelBucketPoints([]);
      return;
    }
    if (!range.start || !range.end) {
      return;
    }

    const controller = new AbortController();
    const hours = Math.max(1, Math.round((range.end.getTime() - range.start.getTime()) / HOUR_MS));
    const params = new URLSearchParams({
      hours: String(hours),
      bucket_size: "3600",
      channel_id: channelFilter,
    });

    setChannelLoading(true);
    fetch(`/api/guilds/${guildId}/analytics/message-buckets?${params.toString()}`, {
      signal: controller.signal,
      credentials: "include",
    })
      .then((res) => (res.ok ? res.json() : null))
      .then((data) => {
        if (!data || !Array.isArray(data.points)) {
          setChannelBucketPoints([]);
          return;
        }
        setChannelBucketPoints(data.points);
      })
      .catch((error) => {
        if (error instanceof Error && error.name === "AbortError") return;
        console.error(error);
      })
      .finally(() => setChannelLoading(false));

    return () => controller.abort();
  }, [channelFilter, guildId, range.end, range.start]);

  const bucketMs = useMemo(() => {
    if (granularity === "hour") return HOUR_MS;
    if (granularity === "day") return DAY_MS;
    if (!range.start || !range.end) return DAY_MS;
    const rangeMs = range.end.getTime() - range.start.getTime();
    return rangeMs <= 2 * DAY_MS ? HOUR_MS : DAY_MS;
  }, [granularity, range.end, range.start]);

  const messageSeries = useMemo(() => {
    if (channelFilter !== "all") {
      return aggregateSeries(
        channelBucketPoints,
        bucketMs,
        range.start,
        range.end,
        (point) => parseIso(point.bucket_start)?.getTime() ?? null,
        (point) => point.count,
      );
    }
    return aggregateSeries(
      messageCounts,
      bucketMs,
      range.start,
      range.end,
      (point) => parseIso(point.time_bucket)?.getTime() ?? null,
      (point) => point.count,
    );
  }, [bucketMs, channelBucketPoints, channelFilter, messageCounts, range.end, range.start]);

  const prevRange = useMemo(() => {
    if (!range.start || !range.end) return null;
    const duration = range.end.getTime() - range.start.getTime();
    if (duration <= 0) return null;
    return { start: new Date(range.start.getTime() - duration), end: range.start };
  }, [range.end, range.start]);

  const messagePrevSeries = useMemo(() => {
    if (!prevRange) return [];
    if (channelFilter !== "all") {
      return aggregateSeries(
        channelBucketPoints,
        bucketMs,
        prevRange.start,
        prevRange.end,
        (point) => parseIso(point.bucket_start)?.getTime() ?? null,
        (point) => point.count,
      );
    }
    return aggregateSeries(
      messageCounts,
      bucketMs,
      prevRange.start,
      prevRange.end,
      (point) => parseIso(point.time_bucket)?.getTime() ?? null,
      (point) => point.count,
    );
  }, [bucketMs, channelBucketPoints, channelFilter, messageCounts, prevRange]);

  const voiceSeries = useMemo(
    () =>
      aggregateSeries(
        voicePoints,
        bucketMs,
        range.start,
        range.end,
        (point) => parseIso(point.bucket_start)?.getTime() ?? null,
        (point) => Math.round(point.total_seconds / 60),
      ),
    [bucketMs, range.end, range.start, voicePoints],
  );

  const voicePrevSeries = useMemo(
    () =>
      prevRange
        ? aggregateSeries(
            voicePoints,
            bucketMs,
            prevRange.start,
            prevRange.end,
            (point) => parseIso(point.bucket_start)?.getTime() ?? null,
            (point) => Math.round(point.total_seconds / 60),
          )
        : [],
    [bucketMs, prevRange, voicePoints],
  );

  const commandSeries = useMemo(
    () =>
      aggregateSeries(
        commandPoints,
        bucketMs,
        range.start,
        range.end,
        (point) => parseIso(point.bucket_start)?.getTime() ?? null,
        (point) => point.count,
      ),
    [bucketMs, commandPoints, range.end, range.start],
  );

  const commandPrevSeries = useMemo(
    () =>
      prevRange
        ? aggregateSeries(
            commandPoints,
            bucketMs,
            prevRange.start,
            prevRange.end,
            (point) => parseIso(point.bucket_start)?.getTime() ?? null,
            (point) => point.count,
          )
        : [],
    [bucketMs, commandPoints, prevRange],
  );

  const heroSeries = useMemo(() => {
    const map = new Map<number, { ts: number; messages: number; voice: number }>();
    messageSeries.forEach((point) => {
      map.set(point.ts, { ts: point.ts, messages: point.value, voice: 0 });
    });
    voiceSeries.forEach((point) => {
      const existing = map.get(point.ts);
      if (existing) {
        existing.voice = point.value;
      } else {
        map.set(point.ts, { ts: point.ts, messages: 0, voice: point.value });
      }
    });
    return Array.from(map.values())
      .sort((a, b) => a.ts - b.ts)
      .map((point) => ({
        ...point,
        label: formatAxisLabel(point.ts, bucketMs),
      }));
  }, [bucketMs, messageSeries, voiceSeries]);

  const summaryInRange = useMemo(() => {
    return summaryPoints.filter((point) => {
      const day = parseIso(point.day);
      if (!day) return false;
      if (!range.start || !range.end) return true;
      return day >= range.start && day <= range.end;
    });
  }, [range.end, range.start, summaryPoints]);

  const latestSummary = summaryInRange[summaryInRange.length - 1];
  const previousSummary = summaryInRange[summaryInRange.length - 2];

  const messagesTotal = sumSeries(messageSeries);
  const voiceTotal = sumSeries(voiceSeries);
  const commandTotal = sumSeries(commandSeries);
  const peakVoice = useMemo(() => {
    return voicePoints.reduce((maxValue, point) => {
      const ts = parseIso(point.bucket_start);
      if (!ts || !clampToRange(ts, range.start, range.end)) return maxValue;
      return Math.max(maxValue, point.peak_concurrent);
    }, 0);
  }, [range.end, range.start, voicePoints]);

  const messagesDelta = deltaPercent(messagesTotal, sumSeries(messagePrevSeries));
  const voiceDelta = deltaPercent(voiceTotal, sumSeries(voicePrevSeries));
  const commandsDelta = deltaPercent(commandTotal, sumSeries(commandPrevSeries));

  const activeChannelsDelta =
    latestSummary && previousSummary
      ? deltaPercent(latestSummary.active_channels, previousSummary.active_channels)
      : null;
  const dauDelta =
    latestSummary && previousSummary && latestSummary.dau_est && previousSummary.dau_est
      ? deltaPercent(latestSummary.dau_est, previousSummary.dau_est)
      : null;

  const insights = useMemo(() => {
    const items: string[] = [];
    if (messagesDelta !== null) {
      items.push(
        messagesDelta >= 0
          ? `Message volume up ${formatPercent(messagesDelta)} vs the previous window.`
          : `Message volume down ${formatPercent(messagesDelta)} vs the previous window.`,
      );
    }
    if (voiceDelta !== null) {
      items.push(
        voiceDelta >= 0
          ? `Voice activity increased ${formatPercent(voiceDelta)}.`
          : `Voice activity decreased ${formatPercent(voiceDelta)}.`,
      );
    }
    if (topChannels.length > 0) {
      items.push(`Top channel is #${topChannels[0].channel_id} with ${formatCompact(topChannels[0].count)} messages.`);
    }
    if (latestSummary?.active_channels) {
      items.push(`${latestSummary.active_channels} channels were active most recently.`);
    }
    return items.slice(0, 4);
  }, [latestSummary?.active_channels, messagesDelta, topChannels, voiceDelta]);

  const emptyState = messageCounts.length === 0 && voicePoints.length === 0 && commandPoints.length === 0;

  return (
    <div className="space-y-8">
      <Card className="rounded-2xl border bg-card/70 p-4">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <CalendarRange className="h-4 w-4" />
            Time range
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {RANGE_PRESETS.map((preset) => (
              <Button
                key={preset.value}
                size="sm"
                variant={rangePreset === preset.value ? "default" : "outline"}
                onClick={() => setRangePreset(preset.value)}
              >
                {preset.label}
              </Button>
            ))}
          </div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <RefreshCw className="h-3 w-3" />
            Updated {now.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })}
          </div>
        </div>

        {rangePreset === "custom" && (
          <div className="mt-3 flex flex-wrap gap-2">
            <Input type="date" value={customStart} onChange={(event) => setCustomStart(event.target.value)} />
            <Input type="date" value={customEnd} onChange={(event) => setCustomEnd(event.target.value)} />
          </div>
        )}

        <div className="mt-4 flex flex-wrap items-center gap-3">
          <div className="text-sm text-muted-foreground">Granularity</div>
          <select
            className="h-9 rounded-md border bg-background px-3 text-sm"
            value={granularity}
            onChange={(event) => setGranularity(event.target.value as Granularity)}
          >
            <option value="auto">Auto</option>
            <option value="hour">Hour</option>
            <option value="day">Day</option>
          </select>
          <div className="text-sm text-muted-foreground">Channel</div>
          <select
            className="h-9 rounded-md border bg-background px-3 text-sm"
            value={channelFilter}
            onChange={(event) => setChannelFilter(event.target.value)}
          >
            <option value="all">All channels</option>
            {topChannels.map((channel) => (
              <option key={channel.channel_id} value={channel.channel_id}>
                #{channel.channel_id}
              </option>
            ))}
          </select>
          {channelLoading && <span className="text-xs text-muted-foreground">Loading...</span>}
          <div className="ml-auto flex flex-wrap items-center gap-2">
            <Button
              size="sm"
              variant={metricMode === "messages" ? "default" : "outline"}
              onClick={() => setMetricMode("messages")}
            >
              Messages
            </Button>
            <Button
              size="sm"
              variant={metricMode === "voice" ? "default" : "outline"}
              onClick={() => setMetricMode("voice")}
            >
              Voice
            </Button>
            <Button
              size="sm"
              variant={metricMode === "both" ? "default" : "outline"}
              onClick={() => setMetricMode("both")}
            >
              Both
            </Button>
          </div>
        </div>
      </Card>

      {emptyState ? (
        <Card className="rounded-2xl border border-dashed bg-muted/20 p-8">
          <div className="text-center">
            <div className="text-lg font-semibold">Analytics are warming up</div>
            <p className="mt-2 text-sm text-muted-foreground">
              We started collecting data recently. New stats appear after the first messages and voice activity.
            </p>
          </div>
          <div className="mt-6 grid gap-4 md:grid-cols-2">
            <div className="rounded-xl border bg-card/60 p-4 text-sm text-muted-foreground">
              <div className="font-medium text-foreground">Checklist</div>
              <ul className="mt-2 space-y-1">
                <li>Bot installed</li>
                <li>Permissions verified</li>
                <li>Waiting for first messages</li>
                <li>Waiting for first voice session</li>
              </ul>
            </div>
            <div className="rounded-xl border bg-card/60 p-4 text-sm text-muted-foreground">
              <div className="font-medium text-foreground">What happens next</div>
              <p className="mt-2">
                Send a few messages and check back in a minute. Data refreshes automatically and powers the charts
                below.
              </p>
            </div>
          </div>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-6">
          <KpiCard
            title="Messages"
            value={messagesTotal}
            delta={messagesDelta}
            icon={BarChart3}
            series={messageSeries.slice(-24)}
          />
          <KpiCard
            title="Active channels"
            value={latestSummary?.active_channels ?? null}
            delta={activeChannelsDelta}
            icon={Users2}
            series={summaryInRange.map((point, index) => ({ ts: index, value: point.active_channels }))}
          />
          <KpiCard
            title="DAU estimate"
            value={latestSummary?.dau_est ?? null}
            delta={dauDelta}
            icon={Users2}
            series={summaryInRange.map((point, index) => ({ ts: index, value: point.dau_est ?? 0 }))}
          />
          <KpiCard
            title="Voice minutes"
            value={voiceTotal}
            delta={voiceDelta}
            icon={Volume2}
            series={voiceSeries.slice(-24)}
          />
          <KpiCard title="Peak VC" value={peakVoice} icon={Activity} footnote="Peak listeners" />
          <KpiCard
            title="Commands"
            value={commandTotal}
            delta={commandsDelta}
            icon={LineChartIcon}
            series={commandSeries.slice(-24)}
          />
        </div>
      )}

      <div className="grid gap-6 lg:grid-cols-[2fr,1fr]">
        <Card className="rounded-2xl border bg-card/70 p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <div className="text-sm text-muted-foreground">Activity over time</div>
              <div className="text-lg font-semibold">Community activity</div>
            </div>
          </div>
          <div className="mt-4 h-[320px]">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={heroSeries}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="label" tickLine={false} axisLine={false} fontSize={12} />
                <YAxis tickLine={false} axisLine={false} fontSize={12} />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "hsl(var(--card))",
                    borderColor: "hsl(var(--border))",
                    color: "hsl(var(--foreground))",
                  }}
                />
                {(metricMode === "messages" || metricMode === "both") && (
                  <Line type="monotone" dataKey="messages" stroke="hsl(var(--primary))" strokeWidth={2} dot={false} />
                )}
                {(metricMode === "voice" || metricMode === "both") && (
                  <Line type="monotone" dataKey="voice" stroke="hsl(var(--accent))" strokeWidth={2} dot={false} />
                )}
              </LineChart>
            </ResponsiveContainer>
          </div>
        </Card>

        <Card className="rounded-2xl border bg-card/70 p-4">
          <div className="text-sm text-muted-foreground">Top channels</div>
          <div className="text-lg font-semibold">Most active</div>
          <div className="mt-4 space-y-3">
            {topChannels.length === 0 ? (
              <div className="rounded-lg border border-dashed bg-muted/20 p-4 text-sm text-muted-foreground">
                No channel activity yet.
              </div>
            ) : (
              topChannels.slice(0, 8).map((channel, index) => (
                <div key={channel.channel_id} className="flex items-center justify-between text-sm">
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-muted-foreground">{index + 1}</span>
                    <span className="font-medium">#{channel.channel_id}</span>
                  </div>
                  <span className="text-muted-foreground">{formatCompact(channel.count)}</span>
                </div>
              ))
            )}
          </div>
        </Card>
      </div>

      <Card className="rounded-2xl border bg-card/70 p-4">
        <div className="flex flex-wrap items-center gap-3">
          {(["messages", "voice", "sentiment", "commands"] as AnalyticsTab[]).map((item) => (
            <Button
              key={item}
              size="sm"
              variant={tab === item ? "default" : "outline"}
              onClick={() => setTab(item)}
            >
              {item.charAt(0).toUpperCase() + item.slice(1)}
            </Button>
          ))}
        </div>

        <div className="mt-6">
          {tab === "messages" && (
            <div className="grid gap-6 lg:grid-cols-[2fr,1fr]">
              <Card className="rounded-2xl border bg-card/70 p-4">
                <div className="text-sm text-muted-foreground">Message volume</div>
                <div className="text-lg font-semibold">Messages over time</div>
                <div className="mt-4 h-[300px]">
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart
                      data={messageSeries.map((point) => ({
                        label: formatAxisLabel(point.ts, bucketMs),
                        count: point.value,
                      }))}
                    >
                      <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                      <XAxis dataKey="label" tickLine={false} axisLine={false} fontSize={12} />
                      <YAxis tickLine={false} axisLine={false} fontSize={12} />
                      <Tooltip
                        contentStyle={{
                          backgroundColor: "hsl(var(--card))",
                          borderColor: "hsl(var(--border))",
                          color: "hsl(var(--foreground))",
                        }}
                      />
                      <Bar dataKey="count" fill="hsl(var(--primary))" radius={[4, 4, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              </Card>
              <Card className="rounded-2xl border bg-card/70 p-4">
                <div className="text-sm text-muted-foreground">Messages by channel</div>
                <div className="text-lg font-semibold">Breakdown</div>
                <div className="mt-4 space-y-3">
                  {topChannels.length === 0 ? (
                    <div className="rounded-lg border border-dashed bg-muted/20 p-4 text-sm text-muted-foreground">
                      No channel data yet.
                    </div>
                  ) : (
                    topChannels.map((channel) => (
                      <div key={channel.channel_id} className="flex items-center justify-between text-sm">
                        <span>#{channel.channel_id}</span>
                        <span className="text-muted-foreground">{formatCompact(channel.count)}</span>
                      </div>
                    ))
                  )}
                </div>
              </Card>
            </div>
          )}

          {tab === "voice" && (
            <div className="grid gap-6 lg:grid-cols-[2fr,1fr]">
              <Card className="rounded-2xl border bg-card/70 p-4">
                <div className="text-sm text-muted-foreground">Voice minutes</div>
                <div className="text-lg font-semibold">Voice activity</div>
                <div className="mt-4 h-[300px]">
                  <ResponsiveContainer width="100%" height="100%">
                    <LineChart
                      data={voiceSeries.map((point) => ({
                        label: formatAxisLabel(point.ts, bucketMs),
                        minutes: point.value,
                      }))}
                    >
                      <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                      <XAxis dataKey="label" tickLine={false} axisLine={false} fontSize={12} />
                      <YAxis tickLine={false} axisLine={false} fontSize={12} />
                      <Tooltip
                        contentStyle={{
                          backgroundColor: "hsl(var(--card))",
                          borderColor: "hsl(var(--border))",
                          color: "hsl(var(--foreground))",
                        }}
                      />
                      <Line type="monotone" dataKey="minutes" stroke="hsl(var(--accent))" strokeWidth={2} dot={false} />
                    </LineChart>
                  </ResponsiveContainer>
                </div>
              </Card>
              <Card className="rounded-2xl border bg-card/70 p-4">
                <div className="text-sm text-muted-foreground">Voice highlights</div>
                <div className="text-lg font-semibold">Peak moments</div>
                <div className="mt-4 space-y-3 text-sm text-muted-foreground">
                  <div className="flex items-center justify-between">
                    <span>Total voice minutes</span>
                    <span>{formatCompact(voiceTotal)}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>Peak concurrent</span>
                    <span>{formatCompact(peakVoice)}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>Last updated</span>
                    <span>{formatDate(now)}</span>
                  </div>
                </div>
              </Card>
            </div>
          )}

          {tab === "sentiment" && (
            <div className="grid gap-6 lg:grid-cols-[2fr,1fr]">
              <SentimentChart data={sentimentPoints} className="rounded-2xl border bg-card/70" />
              <Card className="rounded-2xl border bg-card/70 p-4">
                <div className="text-sm text-muted-foreground">Sentiment summary</div>
                <div className="text-lg font-semibold">Latest reading</div>
                <div className="mt-4 space-y-3 text-sm text-muted-foreground">
                  {sentimentPoints.length === 0 ? (
                    <div className="rounded-lg border border-dashed bg-muted/20 p-4 text-sm text-muted-foreground">
                      No sentiment data yet.
                    </div>
                  ) : (
                    <>
                      <div className="flex items-center justify-between">
                        <span>Latest score</span>
                        <span>{sentimentPoints[sentimentPoints.length - 1]?.score?.toFixed(2) ?? "—"}</span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span>Label</span>
                        <span>{sentimentPoints[sentimentPoints.length - 1]?.sentiment ?? "—"}</span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span>Last updated</span>
                        <span>{formatDate(sentimentPoints[sentimentPoints.length - 1]?.day)}</span>
                      </div>
                    </>
                  )}
                </div>
              </Card>
            </div>
          )}

          {tab === "commands" && (
            <div className="grid gap-6 lg:grid-cols-[2fr,1fr]">
              <Card className="rounded-2xl border bg-card/70 p-4">
                <div className="text-sm text-muted-foreground">Command usage</div>
                <div className="text-lg font-semibold">Commands over time</div>
                <div className="mt-4 h-[300px]">
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart
                      data={commandSeries.map((point) => ({
                        label: formatAxisLabel(point.ts, bucketMs),
                        count: point.value,
                      }))}
                    >
                      <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                      <XAxis dataKey="label" tickLine={false} axisLine={false} fontSize={12} />
                      <YAxis tickLine={false} axisLine={false} fontSize={12} />
                      <Tooltip
                        contentStyle={{
                          backgroundColor: "hsl(var(--card))",
                          borderColor: "hsl(var(--border))",
                          color: "hsl(var(--foreground))",
                        }}
                      />
                      <Bar dataKey="count" fill="hsl(var(--primary))" radius={[4, 4, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              </Card>
              <Card className="rounded-2xl border bg-card/70 p-4">
                <div className="text-sm text-muted-foreground">Top commands</div>
                <div className="text-lg font-semibold">Most used</div>
                <div className="mt-4 space-y-3">
                  {topCommands.length === 0 ? (
                    <div className="rounded-lg border border-dashed bg-muted/20 p-4 text-sm text-muted-foreground">
                      No command usage yet.
                    </div>
                  ) : (
                    topCommands.map((command) => (
                      <div key={command.command_name} className="flex items-center justify-between text-sm">
                        <span>/{command.command_name}</span>
                        <span className="text-muted-foreground">{formatCompact(command.count)}</span>
                      </div>
                    ))
                  )}
                </div>
              </Card>
            </div>
          )}
        </div>
      </Card>

      <Card className="rounded-2xl border bg-card/70 p-4">
        <div className="text-sm text-muted-foreground">Insights</div>
        <div className="text-lg font-semibold">What stands out</div>
        <div className="mt-4 space-y-2 text-sm text-muted-foreground">
          {insights.length === 0 ? (
            <div className="rounded-lg border border-dashed bg-muted/20 p-4 text-sm text-muted-foreground">
              No insights yet. Keep the bot active to unlock trends.
            </div>
          ) : (
            insights.map((insight) => <div key={insight}>{insight}</div>)
          )}
        </div>
      </Card>

      <div className="text-xs text-muted-foreground">
        Data source: guild {guildId}. Metrics update every few minutes.
      </div>
    </div>
  );
}
