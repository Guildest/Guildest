"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  Ban,
  Bot,
  Calendar,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clock,
  FileDown,
  Filter,
  List,
  MessageSquareX,
  Settings,
  ShieldAlert,
  ShieldCheck,
  UserX,
} from "lucide-react";
import { ModerationLogItem, ModerationLogUser } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

type DatePreset = "today" | "24h" | "7d" | "30d" | "custom";
type ViewMode = "list" | "timeline";
type ActorFilter = "all" | "human" | "bot" | "system";

const ACTION_FILTERS = [
  "All actions",
  "Ban",
  "Unban",
  "Kick",
  "Timeout",
  "Warn",
  "Message Delete",
  "Role Change",
  "AutoMod Trigger",
  "Config Change",
  "Review",
  "Other",
] as const;

type ActionFilter = (typeof ACTION_FILTERS)[number];

const DATE_PRESETS: { value: DatePreset; label: string }[] = [
  { value: "today", label: "Today" },
  { value: "24h", label: "Last 24h" },
  { value: "7d", label: "Last 7 days" },
  { value: "30d", label: "Last 30 days" },
  { value: "custom", label: "Custom range" },
];

const ACTOR_FILTERS: { value: ActorFilter; label: string }[] = [
  { value: "all", label: "All actors" },
  { value: "human", label: "Human moderators" },
  { value: "bot", label: "Bots" },
  { value: "system", label: "System" },
];

interface ModerationLogViewProps {
  guildId: string;
  initialLogs: ModerationLogItem[];
  initialUsers?: Record<string, ModerationLogUser>;
  botId?: string | null;
}

type SavedView = {
  name: string;
  filters: {
    datePreset: DatePreset;
    customStart: string;
    customEnd: string;
    actionFilter: ActionFilter;
    actorFilter: ActorFilter;
    targetFilter: string;
    botFilter: string;
    searchQuery: string;
    viewMode: ViewMode;
  };
};

function getActionType(log: ModerationLogItem): ActionFilter {
  const action = (log.action ?? "").toLowerCase();
  const source = (log.source ?? "").toLowerCase();

  if (action.includes("unban")) return "Unban";
  if (action.includes("ban")) return "Ban";
  if (action.includes("kick")) return "Kick";
  if (action.includes("timeout")) return "Timeout";
  if (action.includes("warn")) return "Warn";
  if (action.includes("delete")) return "Message Delete";
  if (action.includes("role")) return "Role Change";
  if (action.includes("config") || action.includes("setting")) return "Config Change";
  if (source === "automod" || action.includes("flagged") || action.includes("automod")) return "AutoMod Trigger";
  if (action.includes("review")) return "Review";
  return "Other";
}

function actionMeta(actionType: ActionFilter) {
  switch (actionType) {
    case "Ban":
      return { icon: Ban, badge: "bg-red-500/10 text-red-400", label: "Ban" };
    case "Unban":
      return { icon: ShieldCheck, badge: "bg-green-500/10 text-green-400", label: "Unban" };
    case "Kick":
      return { icon: UserX, badge: "bg-orange-500/10 text-orange-400", label: "Kick" };
    case "Timeout":
      return { icon: Clock, badge: "bg-orange-500/10 text-orange-400", label: "Timeout" };
    case "Warn":
      return { icon: AlertTriangle, badge: "bg-amber-500/10 text-amber-400", label: "Warn" };
    case "Message Delete":
      return { icon: MessageSquareX, badge: "bg-blue-500/10 text-blue-400", label: "Message Delete" };
    case "Role Change":
      return { icon: ShieldAlert, badge: "bg-sky-500/10 text-sky-400", label: "Role Change" };
    case "AutoMod Trigger":
      return { icon: Bot, badge: "bg-slate-500/10 text-slate-300", label: "AutoMod Trigger" };
    case "Config Change":
      return { icon: Settings, badge: "bg-violet-500/10 text-violet-400", label: "Config Change" };
    case "Review":
      return { icon: CheckCircle2, badge: "bg-emerald-500/10 text-emerald-400", label: "Review" };
    default:
      return { icon: ShieldAlert, badge: "bg-muted text-muted-foreground", label: "Other" };
  }
}

function parseDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

function startOfDay(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function endOfDay(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate(), 23, 59, 59, 999);
}

function formatAbsolute(date: Date) {
  return date.toLocaleString();
}

function formatRelative(date: Date) {
  const diffMs = Date.now() - date.getTime();
  const seconds = Math.round(diffMs / 1000);
  if (seconds < 45) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}

function buildDiscordMessageUrl(guildId: string, channelId?: string | null, messageId?: string | null) {
  if (!channelId || !messageId) return null;
  return `https://discord.com/channels/${guildId}/${channelId}/${messageId}`;
}

export function ModerationLogView({ guildId, initialLogs, initialUsers, botId }: ModerationLogViewProps) {
  const [logItems, setLogItems] = useState<ModerationLogItem[]>(initialLogs);
  const [userMap, setUserMap] = useState<Record<string, ModerationLogUser>>(initialUsers ?? {});
  const [datePreset, setDatePreset] = useState<DatePreset>("7d");
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");
  const [actionFilter, setActionFilter] = useState<ActionFilter>("All actions");
  const [actorFilter, setActorFilter] = useState<ActorFilter>("all");
  const [targetFilter, setTargetFilter] = useState("");
  const [botFilter, setBotFilter] = useState("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>("list");
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());
  const [savedViews, setSavedViews] = useState<SavedView[]>([]);
  const [loading, setLoading] = useState(false);
  const searchRef = useRef<HTMLInputElement | null>(null);

  const storageKey = `guildest:modlog:views:${guildId}`;

  useEffect(() => {
    const raw = window.localStorage.getItem(storageKey);
    if (!raw) return;
    try {
      const parsed = JSON.parse(raw) as SavedView[];
      if (Array.isArray(parsed)) {
        setSavedViews(parsed);
      }
    } catch {
      window.localStorage.removeItem(storageKey);
    }
  }, [storageKey]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key === "/" && document.activeElement?.tagName !== "INPUT") {
        event.preventDefault();
        searchRef.current?.focus();
      }
      if (event.key === "Escape" && document.activeElement?.tagName !== "INPUT") {
        setSearchQuery("");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const getUser = (userId?: string | null) => {
    if (!userId) return null;
    return userMap[userId] ?? null;
  };

  const getUserLabel = (userId?: string | null) => {
    if (!userId) return "Unknown";
    const user = getUser(userId);
    if (!user) return userId;
    return user.global_name || user.username || userId;
  };

  const getUserTag = (userId?: string | null) => {
    if (!userId) return null;
    const user = getUser(userId);
    if (!user || !user.username) return null;
    const discriminator = user.discriminator && user.discriminator !== "0" ? `#${user.discriminator}` : "";
    return `${user.username}${discriminator}`;
  };

  const getAvatarUrl = (userId?: string | null, size: number = 64) => {
    if (!userId) return null;
    const user = getUser(userId);
    if (!user?.avatar) return null;
    return `https://cdn.discordapp.com/avatars/${userId}/${user.avatar}.png?size=${size}`;
  };

  const availableTargets = useMemo(() => {
    const ids = new Set<string>();
    logItems.forEach((log) => {
      const target = log.target_id || log.author_id;
      if (target) ids.add(target);
    });
    return Array.from(ids);
  }, [logItems]);

  const availableBots = useMemo(() => {
    const ids = new Set<string>();
    logItems.forEach((log) => {
      if (log.bot_id) ids.add(log.bot_id);
    });
    if (botId) ids.add(botId);
    return Array.from(ids);
  }, [logItems, botId]);

  const getRange = () => {
    const now = new Date();
    if (datePreset === "today") {
      return { start: startOfDay(now), end: endOfDay(now) };
    }
    if (datePreset === "24h") {
      return { start: new Date(now.getTime() - 24 * 60 * 60 * 1000), end: now };
    }
    if (datePreset === "7d") {
      return { start: new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000), end: now };
    }
    if (datePreset === "30d") {
      return { start: new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000), end: now };
    }
    if (datePreset === "custom") {
      return {
        start: customStart ? startOfDay(new Date(`${customStart}T00:00:00`)) : null,
        end: customEnd ? endOfDay(new Date(`${customEnd}T00:00:00`)) : null,
      };
    }
    return { start: null, end: null };
  };

  useEffect(() => {
    const controller = new AbortController();
    const timer = window.setTimeout(async () => {
      setLoading(true);
      try {
        const params = new URLSearchParams();
        const range = getRange();
        if (range.start) params.set("start", range.start.toISOString());
        if (range.end) params.set("end", range.end.toISOString());
        if (actionFilter !== "All actions") params.set("action_type", actionFilter);
        if (actorFilter !== "all") params.set("actor_type", actorFilter);
        if (targetFilter.trim()) params.set("target_id", targetFilter.trim());
        if (botFilter === "this" && botId) params.set("bot_id", botId);
        if (botFilter !== "all" && botFilter !== "this") params.set("bot_id", botFilter);
        if (searchQuery.trim()) params.set("search", searchQuery.trim());
        params.set("include_users", "true");

        const res = await fetch(`/api/guilds/${guildId}/moderation/logs?${params.toString()}`, {
          signal: controller.signal,
        });
        if (!res.ok) {
          throw new Error(`Failed to fetch logs (${res.status})`);
        }
        const data = await res.json();
        setLogItems(Array.isArray(data.items) ? data.items : []);
        setUserMap(data.users ?? {});
      } catch (error) {
        if (error instanceof Error && error.name === "AbortError") return;
        console.error(error);
      } finally {
        setLoading(false);
      }
    }, 250);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [
    actionFilter,
    actorFilter,
    botFilter,
    botId,
    customEnd,
    customStart,
    datePreset,
    guildId,
    searchQuery,
    targetFilter,
  ]);

  useEffect(() => {
    setExpandedIds(new Set());
  }, [logItems]);

  const filteredLogs = logItems;

  const groupedTimeline = useMemo(() => {
    const groups: Record<string, ModerationLogItem[]> = {};
    filteredLogs.forEach((log) => {
      const date = parseDate(log.created_at);
      const label = date ? date.toLocaleDateString() : "Unknown date";
      if (!groups[label]) groups[label] = [];
      groups[label].push(log);
    });
    return Object.entries(groups).sort((a, b) => {
      const aDate = parseDate(a[1][0]?.created_at || "");
      const bDate = parseDate(b[1][0]?.created_at || "");
      if (!aDate || !bDate) return 0;
      return bDate.getTime() - aDate.getTime();
    });
  }, [filteredLogs]);

  const toggleExpanded = (id: number) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleExport = (format: "csv" | "json") => {
    const rows = filteredLogs.map((log) => ({
      id: log.id,
      action: log.action,
      action_type: getActionType(log),
      reason: log.reason,
      actor_id: log.actor_id,
      actor_type: log.actor_type,
      target_id: log.target_id || log.author_id,
      bot_id: log.bot_id,
      source: log.source,
      channel_id: log.channel_id,
      message_id: log.message_id,
      created_at: log.created_at,
      metadata: log.metadata,
    }));
    let blob: Blob;
    if (format === "json") {
      blob = new Blob([JSON.stringify(rows, null, 2)], { type: "application/json" });
    } else {
      const headers = Object.keys(rows[0] ?? {});
      const csvRows = [
        headers.join(","),
        ...rows.map((row) =>
          headers
            .map((key) => {
              const value = (row as Record<string, unknown>)[key];
              if (value === null || value === undefined) return "";
              const escaped = String(value).replace(/\"/g, '""');
              return `"${escaped}"`;
            })
            .join(",")
        ),
      ];
      blob = new Blob([csvRows.join("\n")], { type: "text/csv" });
    }
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `moderation-logs-${guildId}.${format}`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  };

  const saveView = () => {
    const name = window.prompt("Save this filter as...");
    if (!name) return;
    const next = [
      ...savedViews.filter((view) => view.name !== name),
      {
        name,
        filters: {
          datePreset,
          customStart,
          customEnd,
          actionFilter,
          actorFilter,
          targetFilter,
          botFilter,
          searchQuery,
          viewMode,
        },
      },
    ];
    setSavedViews(next);
    window.localStorage.setItem(storageKey, JSON.stringify(next));
  };

  const loadView = (name: string) => {
    const view = savedViews.find((item) => item.name === name);
    if (!view) return;
    setDatePreset(view.filters.datePreset);
    setCustomStart(view.filters.customStart);
    setCustomEnd(view.filters.customEnd);
    setActionFilter(view.filters.actionFilter);
    setActorFilter(view.filters.actorFilter);
    setTargetFilter(view.filters.targetFilter);
    setBotFilter(view.filters.botFilter);
    setSearchQuery(view.filters.searchQuery);
    setViewMode(view.filters.viewMode);
  };

  const deleteView = (name: string) => {
    const next = savedViews.filter((view) => view.name !== name);
    setSavedViews(next);
    window.localStorage.setItem(storageKey, JSON.stringify(next));
  };

  const clearFilters = () => {
    setDatePreset("7d");
    setCustomStart("");
    setCustomEnd("");
    setActionFilter("All actions");
    setActorFilter("all");
    setTargetFilter("");
    setBotFilter("all");
    setSearchQuery("");
  };

  return (
    <div className="space-y-6">
      <div className="rounded-2xl border bg-card p-4">
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Filter className="h-4 w-4" />
            Filters
          </div>
          {loading && <span className="text-xs text-muted-foreground">Refreshing...</span>}
          <div className="flex flex-wrap gap-2">
            {savedViews.length > 0 && (
              <select
                className="h-9 rounded-md border bg-background px-3 text-sm"
                onChange={(event) => {
                  if (!event.target.value) return;
                  loadView(event.target.value);
                }}
                value=""
              >
                <option value="">Saved views</option>
                {savedViews.map((view) => (
                  <option key={view.name} value={view.name}>
                    {view.name}
                  </option>
                ))}
              </select>
            )}
            <Button variant="outline" size="sm" onClick={saveView}>
              Save view
            </Button>
          </div>
        </div>

        <div className="mt-4 grid gap-3 md:grid-cols-6">
          <div className="flex items-center gap-2 rounded-md border bg-background px-3 py-2 text-sm">
            <Calendar className="h-4 w-4 text-muted-foreground" />
            <select
              className="w-full bg-transparent text-sm outline-none"
              value={datePreset}
              onChange={(event) => setDatePreset(event.target.value as DatePreset)}
            >
              {DATE_PRESETS.map((preset) => (
                <option key={preset.value} value={preset.value}>
                  {preset.label}
                </option>
              ))}
            </select>
          </div>

          <select
            className="h-10 rounded-md border bg-background px-3 text-sm"
            value={actionFilter}
            onChange={(event) => setActionFilter(event.target.value as ActionFilter)}
          >
            {ACTION_FILTERS.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>

          <select
            className="h-10 rounded-md border bg-background px-3 text-sm"
            value={actorFilter}
            onChange={(event) => setActorFilter(event.target.value as ActorFilter)}
          >
            {ACTOR_FILTERS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>

          <div>
            <Input
              placeholder="Target ID"
              value={targetFilter}
              onChange={(event) => setTargetFilter(event.target.value)}
              list="moderation-targets"
            />
            <datalist id="moderation-targets">
              {availableTargets.map((target) => (
                <option key={target} value={target} label={getUserLabel(target)} />
              ))}
            </datalist>
          </div>

          <select
            className="h-10 rounded-md border bg-background px-3 text-sm"
            value={botFilter}
            onChange={(event) => setBotFilter(event.target.value)}
          >
            <option value="all">All bots</option>
            <option value="this">This bot only</option>
            {availableBots.map((id) => (
              <option key={id} value={id}>
                Bot {getUserLabel(id)}
              </option>
            ))}
          </select>

          <Input
            ref={searchRef}
            placeholder="Search..."
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
          />
        </div>

        {datePreset === "custom" && (
          <div className="mt-3 flex flex-wrap gap-2">
            <Input
              type="date"
              value={customStart}
              onChange={(event) => setCustomStart(event.target.value)}
            />
            <Input
              type="date"
              value={customEnd}
              onChange={(event) => setCustomEnd(event.target.value)}
            />
          </div>
        )}

        <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap gap-2">
            <Button
              variant={viewMode === "list" ? "default" : "outline"}
              size="sm"
              onClick={() => setViewMode("list")}
            >
              <List className="mr-2 h-4 w-4" />
              List view
            </Button>
            <Button
              variant={viewMode === "timeline" ? "default" : "outline"}
              size="sm"
              onClick={() => setViewMode("timeline")}
            >
              <Clock className="mr-2 h-4 w-4" />
              Timeline view
            </Button>
            <Button variant="outline" size="sm" onClick={clearFilters}>
              Clear filters
            </Button>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" size="sm" onClick={() => handleExport("csv")}>
              <FileDown className="mr-2 h-4 w-4" />
              Export CSV
            </Button>
            <Button variant="outline" size="sm" onClick={() => handleExport("json")}>
              <FileDown className="mr-2 h-4 w-4" />
              Export JSON
            </Button>
          </div>
        </div>

        {savedViews.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-2 text-xs text-muted-foreground">
            {savedViews.map((view) => (
              <div key={view.name} className="flex items-center gap-2 rounded-full border px-3 py-1">
                <button
                  type="button"
                  className="text-xs font-medium text-foreground"
                  onClick={() => loadView(view.name)}
                >
                  {view.name}
                </button>
                <button type="button" onClick={() => deleteView(view.name)}>
                  x
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {filteredLogs.length === 0 ? (
        <div className="rounded-2xl border border-dashed bg-muted/20 p-8 text-center text-muted-foreground">
          No moderation actions in the selected timeframe.
        </div>
      ) : viewMode === "timeline" ? (
        <div className="space-y-6">
          {groupedTimeline.map(([label, items]) => (
            <div key={label} className="space-y-3">
              <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {label} - {items.length} actions
              </div>
              <div className="space-y-2">
                {items.map((log) => {
                  const type = getActionType(log);
                  const meta = actionMeta(type);
                  const Icon = meta.icon;
                  const createdAt = parseDate(log.created_at);
                  const reason = log.reason || "No reason provided";
                  return (
                    <div
                      key={log.id}
                      className="flex flex-col gap-2 rounded-xl border bg-card/70 p-4 md:flex-row md:items-center md:justify-between"
                    >
                      <div className="flex items-center gap-3">
                        <span className={cn("rounded-full p-2", meta.badge)}>
                          <Icon className="h-4 w-4" />
                        </span>
                        <div>
                          <p className="text-sm font-semibold">{meta.label}</p>
                          <p className="text-xs text-muted-foreground">{reason}</p>
                        </div>
                      </div>
                      <div className="text-xs text-muted-foreground">
                        {createdAt ? formatRelative(createdAt) : "unknown time"}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="space-y-3">
          {filteredLogs.map((log) => {
            const type = getActionType(log);
            const meta = actionMeta(type);
            const Icon = meta.icon;
            const createdAt = parseDate(log.created_at);
            const actorType = (log.actor_type ?? "system").toLowerCase();
            const targetId = log.target_id || log.author_id;
            const targetLabel = targetId ? getUserLabel(targetId) : "Unknown";
            const targetTag = targetId ? getUserTag(targetId) : null;
            const targetAvatar = getAvatarUrl(targetId, 48);
            const actorId = log.actor_id || null;
            const actorLabel = actorId ? getUserLabel(actorId) : "System";
            const actorTag = actorId ? getUserTag(actorId) : null;
            const messageLink = buildDiscordMessageUrl(guildId, log.channel_id, log.message_id);
            const content =
              log.metadata && typeof log.metadata === "object"
                ? (log.metadata as Record<string, unknown>)["message_content"]
                : null;
            const expanded = expandedIds.has(log.id);
            return (
              <div key={log.id} id={`log-${log.id}`} className="rounded-2xl border bg-card/70">
                <button
                  type="button"
                  onClick={() => toggleExpanded(log.id)}
                  className="flex w-full flex-col gap-3 px-4 py-4 text-left md:flex-row md:items-center"
                >
                  <div className="flex items-center gap-3 md:w-64">
                    <span className={cn("rounded-full p-2", meta.badge)}>
                      <Icon className="h-4 w-4" />
                    </span>
                    <div>
                      <p className="text-sm font-semibold">{meta.label}</p>
                      <p className="text-xs text-muted-foreground">
                        {log.action || "unknown action"}
                      </p>
                    </div>
                  </div>

                  <div className="flex-1 space-y-2">
                    <div className="flex items-center gap-3">
                      {targetAvatar ? (
                        <img
                          src={targetAvatar}
                          alt={targetLabel}
                          className="h-8 w-8 rounded-full border"
                        />
                      ) : (
                        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-secondary text-xs font-semibold text-secondary-foreground">
                          {targetLabel.charAt(0).toUpperCase()}
                        </div>
                      )}
                      <div>
                        <p className="text-sm font-medium">Target: {targetLabel}</p>
                        <p className="text-xs text-muted-foreground">
                          {targetTag ?? targetId ?? "Unknown ID"}
                        </p>
                      </div>
                    </div>
                    <p className="text-xs text-muted-foreground">
                      {log.reason || "No reason provided"}
                    </p>
                  </div>

                  <div className="text-xs text-muted-foreground md:w-52">
                    <div>Actor: {actorLabel}</div>
                    {actorTag && <div>{actorTag}</div>}
                    <div>Type: {actorType}</div>
                  </div>

                  <div className="flex items-center gap-2 text-xs text-muted-foreground md:justify-end md:w-28">
                    <span title={createdAt ? formatAbsolute(createdAt) : log.created_at}>
                      {createdAt ? formatRelative(createdAt) : "unknown"}
                    </span>
                    {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
                  </div>
                </button>

                {expanded && (
                  <div className="border-t px-4 pb-4 pt-3 text-sm text-muted-foreground">
                    <div className="grid gap-3 md:grid-cols-2">
                      <div>
                        <p className="text-xs uppercase tracking-wide">Source</p>
                        <p>{log.source || "unknown"}</p>
                      </div>
                      <div>
                        <p className="text-xs uppercase tracking-wide">Action ID</p>
                        <p>{log.id}</p>
                      </div>
                      <div>
                        <p className="text-xs uppercase tracking-wide">Channel</p>
                        <p>{log.channel_id || "unknown"}</p>
                      </div>
                      <div>
                        <p className="text-xs uppercase tracking-wide">Message</p>
                        <p>{log.message_id || "n/a"}</p>
                      </div>
                      <div>
                        <p className="text-xs uppercase tracking-wide">Bot ID</p>
                        <p>{log.bot_id || "n/a"}</p>
                      </div>
                      <div>
                        <p className="text-xs uppercase tracking-wide">Target ID</p>
                        <p>{targetId || "n/a"}</p>
                      </div>
                    </div>
                    {content && typeof content === "string" && (
                      <div className="mt-3">
                        <p className="text-xs uppercase tracking-wide">Message content</p>
                        <p className="whitespace-pre-wrap">{content}</p>
                      </div>
                    )}
                    <div className="mt-4 flex flex-wrap gap-2">
                      {messageLink && (
                        <a
                          href={messageLink}
                          target="_blank"
                          rel="noreferrer"
                          className="rounded-md border px-3 py-1 text-xs text-foreground hover:border-primary/50"
                        >
                          Jump to message
                        </a>
                      )}
                      <button
                        type="button"
                        className="rounded-md border px-3 py-1 text-xs text-foreground hover:border-primary/50"
                        onClick={() => {
                          const link = `${window.location.origin}${window.location.pathname}#log-${log.id}`;
                          navigator.clipboard?.writeText(link);
                        }}
                      >
                        Copy event link
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
