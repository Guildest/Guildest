import { cookies } from "next/headers";
import { ActivityDistributionChart, HourlyActivityChart } from "@/components/charts";
import { DashboardNav } from "@/components/dashboard-nav";
import { GuildSidebar } from "@/components/guild-sidebar";

import {
  getDashboardMe,
  getGuildHotspots,
} from "@/lib/public-api";

type HotspotsPageProps = {
  searchParams: Promise<{
    guild_id?: string;
  }>;
};

export default async function HotspotsPage({ searchParams }: HotspotsPageProps) {
  const params = await searchParams;
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const dashboard = await getDashboardMe(cookieHeader);

  const accessibleGuilds = dashboard?.accessible_guilds ?? [];
  const selectedGuild =
    accessibleGuilds.find((guild) => guild.guild_id === params.guild_id) ??
    accessibleGuilds[0] ??
    null;
  const hotspots = selectedGuild
    ? await getGuildHotspots(selectedGuild.guild_id, cookieHeader)
    : null;
  const topChannel = hotspots?.top_channels[0] ?? null;
  const healthiestChannel = hotspots?.top_channels
    .slice()
    .sort((a, b) => b.health_score - a.health_score)[0] ?? null;
  const fastestChannel = hotspots?.top_channels
    .filter((channel) => channel.avg_response_seconds !== null)
    .sort((a, b) => (a.avg_response_seconds ?? Number.MAX_SAFE_INTEGER) - (b.avg_response_seconds ?? Number.MAX_SAFE_INTEGER))[0] ?? null;
  const retentionDriver = hotspots?.retention_channels
    .slice()
    .sort((a, b) => b.retention_score - a.retention_score)[0] ?? null;
  const channelData =
    hotspots?.top_channels.map((channel) => ({
      name: channel.label.replace(/^#/, ""),
      value: channel.message_count,
    })) ?? [];

  return (
    <main className="min-h-screen bg-plum px-6 py-8 lg:px-8">
      <div className="mx-auto flex max-w-[1280px] flex-col gap-6">
        {dashboard && accessibleGuilds.length > 0 && (
          <DashboardNav guildId={selectedGuild?.guild_id} />
        )}

        <div className="grid gap-6 lg:grid-cols-[260px_1fr]">
          {dashboard && accessibleGuilds.length > 0 && (
            <GuildSidebar
              accessibleGuilds={accessibleGuilds}
              basePath="/dashboard/hotspots"
              dashboard={dashboard}
              selectedGuild={selectedGuild}
            />
          )}

          <div className="flex flex-col gap-6">
            {/* Highlight Cards */}
            <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
              <div className="metric-card p-5">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                  Most Active Channel
                </p>
                <p className="mt-3 text-2xl font-semibold tracking-tight text-tan">
                  {topChannel?.label ?? "No data"}
                </p>
                <p className="mt-1.5 text-xs text-cream/30">
                  {topChannel
                    ? `${topChannel.message_count.toLocaleString()} messages this week`
                    : "No indexed channel activity yet"}
                </p>
              </div>

              <div className="metric-card p-5">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                  Healthiest Channel
                </p>
                <p className="mt-3 text-2xl font-semibold tracking-tight text-cream">
                  {healthiestChannel?.label ?? "No data"}
                </p>
                <p className="mt-1.5 text-xs text-cream/30">
                  {healthiestChannel
                    ? `${healthiestChannel.health_score.toFixed(1)} / 100 health score`
                    : "Health scores populate once a channel has traffic"}
                </p>
              </div>

              <div className="metric-card p-5">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                  Fastest Response
                </p>
                <p className="mt-3 text-2xl font-semibold tracking-tight text-cream">
                  {fastestChannel?.avg_response_seconds != null
                    ? `${Math.round(fastestChannel.avg_response_seconds)}s`
                    : "No data"}
                </p>
                <p className="mt-1.5 text-xs text-cream/30">
                  {fastestChannel
                    ? `${fastestChannel.label} average reply gap`
                    : "Response timing will populate as channel history grows"}
                </p>
              </div>

              <div className="metric-card p-5">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                  Top Retention Driver
                </p>
                <p className="mt-3 text-2xl font-semibold tracking-tight text-tan">
                  {retentionDriver?.label ?? "No data"}
                </p>
                <p className="mt-1.5 text-xs text-cream/30">
                  {retentionDriver
                    ? `${retentionDriver.d7_retained_members.toLocaleString()} D7 and ${retentionDriver.d30_retained_members.toLocaleString()} D30 retained members`
                    : "Retention contribution appears once cohorts mature"}
                </p>
              </div>
            </div>

            {/* Charts Row */}
            <div className="grid gap-6 md:grid-cols-2">
              <section className="card p-6">
                <div className="mb-6">
                  <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                    Distribution
                  </p>
                  <h3 className="text-lg font-semibold tracking-tight text-cream">
                    Channel Share
                  </h3>
                </div>
                <ActivityDistributionChart data={channelData} />
              </section>

              <section className="card p-6">
                <div className="mb-6">
                  <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                    Timing
                  </p>
                  <h3 className="text-lg font-semibold tracking-tight text-cream">
                    Hourly Activity
                  </h3>
                </div>
                <HourlyActivityChart data={hotspots?.hourly_activity ?? []} />
              </section>
            </div>

            {/* Top Channels Ranking */}
            <section className="card p-6">
              <div className="mb-6">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                  Ranking
                </p>
                <h3 className="text-lg font-semibold tracking-tight text-cream">
                  Top Channels
                </h3>
              </div>

              <div className="flex flex-col gap-2">
                {(hotspots?.top_channels ?? []).map((channel, index) => (
                  <div
                    key={channel.channel_id}
                    className="flex items-center justify-between rounded-xl px-4 py-3 transition-colors hover:bg-surface-light"
                  >
                    <div className="flex items-center gap-3">
                      <span
                        className={`flex h-7 w-7 items-center justify-center rounded-lg text-[11px] font-semibold ${
                          index === 0
                            ? "bg-tan/20 text-tan"
                            : index === 1
                              ? "bg-sand/10 text-sand/70"
                              : index === 2
                                ? "bg-brown/20 text-brown"
                                : "bg-surface-light text-cream/30"
                        }`}
                      >
                        {index + 1}
                      </span>
                      <div>
                        <span className="text-sm font-medium text-cream/80">{channel.label}</span>
                        <p className="text-[11px] text-cream/30">
                          {channel.unique_senders.toLocaleString()} unique senders,{" "}
                          {channel.replies.toLocaleString()} replies
                        </p>
                      </div>
                    </div>
                    <div className="flex items-center gap-4">
                      <span className="w-16 text-right text-[11px] text-cream/30">
                        {channel.avg_response_seconds != null
                          ? `${Math.round(channel.avg_response_seconds)}s avg`
                          : "n/a"}
                      </span>
                      <span
                        className={`w-14 text-right text-[11px] font-medium ${
                          (channel.trend_percent_change ?? 0) >= 0
                            ? "text-emerald-400/70"
                            : "text-red-400/70"
                        }`}
                      >
                        {channel.trend_percent_change != null
                          ? `${channel.trend_percent_change >= 0 ? "+" : ""}${Math.round(channel.trend_percent_change)}%`
                          : "new"}
                      </span>
                      <div className="h-1.5 w-24 rounded-full bg-surface-light overflow-hidden">
                        <div
                          className="h-full bg-tan/60 rounded-full transition-all"
                          style={{
                            width: `${(channel.message_count / (topChannel?.message_count || 1)) * 100}%`,
                          }}
                        />
                      </div>
                      <span className="w-16 text-right text-xs font-mono text-cream/40">
                        {channel.message_count.toLocaleString()}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </section>

            {/* Channel Health */}
            <section className="card p-6">
              <div className="mb-6">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                  Health
                </p>
                <h3 className="text-lg font-semibold tracking-tight text-cream">
                  Channel Health Signals
                </h3>
              </div>

              <div className="grid gap-3 md:grid-cols-2">
                {(hotspots?.top_channels ?? []).map((channel) => (
                  <div
                    key={`${channel.channel_id}-health`}
                    className="card-inner p-4"
                  >
                    <div className="flex items-center justify-between gap-4">
                      <div>
                        <p className="text-sm font-medium text-cream/80">{channel.label}</p>
                        <p className="mt-0.5 text-[11px] text-cream/30">Health score</p>
                      </div>
                      <p className="text-2xl font-semibold tracking-tight text-tan">
                        {channel.health_score.toFixed(1)}
                      </p>
                    </div>
                    <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-surface">
                      <div
                        className="h-full rounded-full bg-tan/60 transition-all"
                        style={{ width: `${Math.min(channel.health_score, 100)}%` }}
                      />
                    </div>
                    <div className="mt-3 grid grid-cols-3 gap-3 text-[11px]">
                      <div>
                        <p className="text-cream/30">Breadth</p>
                        <p className="mt-0.5 text-cream/60 font-medium">
                          {channel.messages_per_sender != null
                            ? `${channel.messages_per_sender.toFixed(1)} msg/sender`
                            : "n/a"}
                        </p>
                      </div>
                      <div>
                        <p className="text-cream/30">Trend</p>
                        <p className={`mt-0.5 font-medium ${
                          (channel.trend_percent_change ?? 0) >= 0 ? "text-emerald-400/70" : "text-red-400/70"
                        }`}>
                          {channel.trend_percent_change != null
                            ? `${channel.trend_percent_change >= 0 ? "+" : ""}${Math.round(channel.trend_percent_change)}%`
                            : "new"}
                        </p>
                      </div>
                      <div>
                        <p className="text-cream/30">Prior period</p>
                        <p className="mt-0.5 text-cream/60 font-medium">
                          {channel.previous_period_messages.toLocaleString()}
                        </p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </section>

            {/* Retention Channels */}
            <section className="card p-6">
              <div className="mb-6">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                  Retention
                </p>
                <h3 className="text-lg font-semibold tracking-tight text-cream">
                  Channels Creating Retention
                </h3>
              </div>

              <div className="grid gap-3 md:grid-cols-2">
                {(hotspots?.retention_channels ?? []).map((channel) => (
                  <div
                    key={`${channel.channel_id}-retention`}
                    className="card-inner p-4"
                  >
                    <div className="flex items-center justify-between gap-4">
                      <div>
                        <p className="text-sm font-medium text-cream/80">{channel.label}</p>
                        <p className="mt-0.5 text-[11px] text-cream/30">
                          Retained member contribution
                        </p>
                      </div>
                      <p className="text-2xl font-semibold tracking-tight text-tan">
                        {channel.retention_score}
                      </p>
                    </div>
                    <div className="mt-3 grid grid-cols-2 gap-3 text-[11px]">
                      <div>
                        <p className="text-cream/30">D7 retained</p>
                        <p className="mt-0.5 text-cream/60 font-medium">
                          {channel.d7_retained_members.toLocaleString()}
                        </p>
                      </div>
                      <div>
                        <p className="text-cream/30">D30 retained</p>
                        <p className="mt-0.5 text-cream/60 font-medium">
                          {channel.d30_retained_members.toLocaleString()}
                        </p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </section>
          </div>
        </div>
      </div>
    </main>
  );
}
