import { backendFetch } from "@/lib/backend.server";
import { DashboardOverview } from "@/lib/types";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Check, Shield, BarChart3, Settings, Smile, Lock, Calendar, Zap, ScrollText, TrendingUp, TrendingDown, Minus, Activity, Clock, MessageSquare, ArrowUpRight, ArrowDownRight } from "lucide-react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

async function getOverview(guildId: string): Promise<DashboardOverview | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/dashboard/overview`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

const FEATURE_INFO: Record<string, { label: string; description: string; icon: React.ElementType }> = {
  moderation_logs: {
    label: "Moderation Logs",
    description: "Keep track of all moderation actions and audit logs.",
    icon: ScrollText,
  },
  sentiment_reports: {
    label: "Sentiment Reports",
    description: "Analyze community mood and sentiment trends over time.",
    icon: Smile,
  },
  event_recommendations: {
    label: "Event Recommendations",
    description: "Get AI-powered suggestions for community events.",
    icon: Calendar,
  },
  analytics_extended: {
    label: "Extended Analytics",
    description: "Access deeper insights and longer data retention.",
    icon: BarChart3,
  },
};

function formatNumber(num: number): string {
  if (num >= 1000000) return (num / 1000000).toFixed(1) + "M";
  if (num >= 1000) return (num / 1000).toFixed(1) + "K";
  return num.toLocaleString();
}

function getTrendIcon(trend: "up" | "down" | "stable" | null) {
  switch (trend) {
    case "up":
      return <TrendingUp className="h-4 w-4 text-green-500" />;
    case "down":
      return <TrendingDown className="h-4 w-4 text-red-500" />;
    case "stable":
      return <Minus className="h-4 w-4 text-muted-foreground" />;
    default:
      return <Minus className="h-4 w-4 text-muted-foreground" />;
  }
}

export default async function GuildOverviewPage({
  params,
}: {
  params: Promise<{ guildId: string }>;
}) {
  const { guildId } = await params;
  const overview = await getOverview(guildId);

  if (!overview) {
    return <div>Failed to load guild overview.</div>;
  }

  const { stats } = overview;
  const isFreePlan = overview.plan === "free";
  const avgDailyMessages = stats?.messages_7d ? Math.round(stats.messages_7d / 7) : 0;
  const avgDailyModActions = stats?.moderation_actions_7d ? Math.round(stats.moderation_actions_7d / 7) : 0;

  return (
    <div className="space-y-6 max-w-7xl mx-auto">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Dashboard Overview</h1>
          <p className="text-muted-foreground mt-1">
            Guild ID: {overview.guild_id} • Plan: <span className="uppercase font-semibold text-primary">{overview.plan}</span>
          </p>
        </div>
        {isFreePlan && (
           <Link href={`/dashboard/${guildId}/billing`}>
            <Button variant="default" size="sm">
              Upgrade Plan
            </Button>
          </Link>
        )}
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <Link href={`/dashboard/${guildId}/analytics`}>
          <Card className="hover:border-primary/50 transition-all hover:shadow-md cursor-pointer h-full">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Messages Today</CardTitle>
              <MessageSquare className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="flex items-baseline gap-2">
                <div className="text-2xl font-bold">
                  {formatNumber(stats?.messages_24h ?? 0)}
                </div>
                {stats?.messages_7d && stats.messages_7d > 0 && (
                  <span className={`text-xs font-medium ${stats.messages_24h / (stats.messages_7d / 7) >= 1 ? "text-green-500" : "text-red-500"}`}>
                    {stats.messages_24h / (stats.messages_7d / 7) >= 1 ? <ArrowUpRight className="h-3 w-3 inline" /> : <ArrowDownRight className="h-3 w-3 inline" />}
                    vs avg
                  </span>
                )}
              </div>
              <p className="text-xs text-muted-foreground mt-1">
                Avg {formatNumber(avgDailyMessages)}/day this week
              </p>
            </CardContent>
          </Card>
        </Link>

        <Link href={`/dashboard/${guildId}/moderation`}>
          <Card className={`transition-all hover:shadow-md cursor-pointer h-full ${!overview.features.moderation_logs ? "opacity-80 bg-muted/20" : "hover:border-primary/50"}`}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Mod Actions Today</CardTitle>
              <Shield className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="flex items-baseline gap-2">
                <div className="text-2xl font-bold">
                  {stats?.moderation_actions_24h ?? 0}
                </div>
              </div>
              <p className="text-xs text-muted-foreground mt-1">
                Avg {avgDailyModActions}/day this week
              </p>
            </CardContent>
          </Card>
        </Link>

        <Link href={`/dashboard/${guildId}/analytics`}>
          <Card className={`transition-all hover:shadow-md cursor-pointer h-full ${!overview.features.sentiment_reports ? "opacity-80 bg-muted/20" : "hover:border-primary/50"}`}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Community Mood</CardTitle>
              <Smile className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              {!overview.features.sentiment_reports ? (
                <div className="flex flex-col gap-1 py-1">
                   <div className="flex items-center gap-2 text-muted-foreground">
                     <Lock className="h-4 w-4" />
                     <span className="font-medium text-sm">Premium</span>
                   </div>
                   <p className="text-xs text-muted-foreground">Sentiment tracking locked</p>
                </div>
              ) : (
                <>
                  <div className="flex items-center gap-2">
                    <div className="text-2xl font-bold capitalize">
                      {stats?.sentiment_label ?? "N/A"}
                    </div>
                    {stats?.sentiment_trend && getTrendIcon(stats.sentiment_trend)}
                  </div>
                  <p className="text-xs text-muted-foreground mt-1">
                    {stats && stats.sentiment_score !== null ? `Score: ${stats.sentiment_score.toFixed(2)}` : "No data"}
                  </p>
                </>
              )}
            </CardContent>
          </Card>
        </Link>

        <Link href={`/dashboard/${guildId}/settings`}>
          <Card className="hover:border-primary/50 transition-all hover:shadow-md cursor-pointer h-full">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Quick Actions</CardTitle>
              <Settings className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="flex items-center gap-2">
                <div className="text-2xl font-bold">Configure</div>
                <ArrowUpRight className="h-4 w-4 text-muted-foreground" />
              </div>
              <p className="text-xs text-muted-foreground mt-1">
                Bot settings and modules
              </p>
            </CardContent>
          </Card>
        </Link>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Weekly Messages</CardTitle>
            <Clock className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {formatNumber(stats?.messages_7d ?? 0)}
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              Last 7 days total
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Monthly Messages</CardTitle>
            <Calendar className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {formatNumber(stats?.messages_30d ?? 0)}
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              Last 30 days total
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Weekly Mod Actions</CardTitle>
            <Activity className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {stats?.moderation_actions_7d ?? 0}
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              Actions in last 7 days
            </p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Feature Status</CardTitle>
          <CardDescription>
            Features enabled for this guild based on your plan.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 md:grid-cols-2">
            {Object.entries(overview.features).map(([key, enabled]) => {
              const info = FEATURE_INFO[key] || { label: key, description: "", icon: Zap };
              const Icon = info.icon;

              return (
                <div
                  key={key}
                  className={`flex items-start gap-3 p-4 rounded-lg border transition-all ${enabled ? "bg-card hover:border-primary/50" : "bg-muted/50 opacity-80"}`}
                >
                  <div className={`p-2 rounded-md ${enabled ? "bg-primary/10 text-primary" : "bg-muted text-muted-foreground"}`}>
                    <Icon className="h-5 w-5" />
                  </div>
                  <div className="flex-1 space-y-1">
                    <div className="flex items-center justify-between">
                      <p className="font-medium leading-none text-sm">{info.label}</p>
                      {enabled ? (
                        <span className="flex items-center gap-1 text-xs font-medium text-primary">
                          <Check className="h-3 w-3" /> Enabled
                        </span>
                      ) : (
                        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
                          <Lock className="h-3 w-3" /> Locked
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-muted-foreground">
                      {info.description}
                    </p>
                  </div>
                </div>
              );
            })}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
