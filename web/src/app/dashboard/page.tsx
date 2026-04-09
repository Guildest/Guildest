import { cookies } from "next/headers";
import { MessageActivityChart } from "@/components/charts";
import { DeadLetterActions } from "@/components/dead-letter-actions";
import { DashboardNav } from "@/components/dashboard-nav";
import { GuildSidebar } from "@/components/guild-sidebar";

import {
  getDashboardMe,
  getGuildActivationFunnel,
  getGuildHealthSummary,
  getGuildMessageSummary,
  getGuildPipelineDiscardHistory,
  getGuildPipelineIncidents,
  getGuildPipelineHealth,
  getGuildPipelineReplayHistory,
  getGuildRetentionCohorts,
  getPublicLinks,
} from "@/lib/public-api";

type DashboardPageProps = {
  searchParams: Promise<{
    guild_id?: string;
    needs_invite?: string;
    permissions?: string;
    reason?: string;
    status?: string;
  }>;
};

type DashboardState = {
  body: string;
  title: string;
  bgClass: string;
};

function getDashboardState(params: {
  needs_invite?: string;
  reason?: string;
  status?: string;
}): DashboardState {
  switch (params.status) {
    case "installed":
      return {
        title: "Guildest is installed and syncing.",
        body:
          "The bot has been added successfully. Historical indexing and live event ingestion can now populate your analytics dashboard.",
        bgClass: "bg-emerald-500/10 border-emerald-500/20",
      };
    case "logged-in":
      return {
        title: "Invite Guildest to unlock analytics.",
        body:
          params.needs_invite === "1"
            ? "You are authenticated. Invite the bot to at least one server where you have Administrator permission to view analytics."
            : "You are authenticated. Invite the bot to a server to start tracking activity.",
        bgClass: "bg-tan/10 border-tan/20",
      };
    case "error":
      return {
        title: "Discord authorization did not complete.",
        body:
          params.reason === "discord-denied"
            ? "Discord denied the authorization request. Try again when you are ready to continue the flow."
            : "The authorization flow did not complete successfully. Retry from the landing page.",
        bgClass: "bg-red-500/10 border-red-500/20",
      };
    case "ready":
      return {
        title: "Your accessible Guildest servers are ready.",
        body:
          "Guildest filtered your Discord servers down to the ones where the bot is installed and your account has Administrator access.",
        bgClass: "bg-surface-light border-border-light",
      };
    default:
      return {
        title: "Log in and invite Guildest to start collecting data.",
        body:
          "Once authenticated, Guildest can match your Discord memberships against installed bot guilds and only expose dashboards where you have Administrator access.",
        bgClass: "bg-surface-light border-border-light",
      };
  }
}

function formatDuration(seconds: number) {
  if (seconds <= 0) return "0s";
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${(seconds / 3600).toFixed(1)}h`;
}

function formatTimestamp(timestamp: string) {
  const parsed = new Date(timestamp);
  if (Number.isNaN(parsed.getTime())) return timestamp;
  return `${parsed.toISOString().replace("T", " ").slice(0, 16)} UTC`;
}

function StatusBadge({ status }: { status: string | null | undefined }) {
  let classes: string;
  switch (status) {
    case "critical":
      classes = "bg-red-500/10 text-red-400 border-red-500/20";
      break;
    case "warning":
      classes = "bg-amber-500/10 text-amber-400 border-amber-500/20";
      break;
    default:
      classes = "bg-emerald-500/10 text-emerald-400 border-emerald-500/20";
      break;
  }
  return (
    <span className={`inline-flex items-center rounded-md border px-2 py-0.5 text-[11px] font-semibold ${classes}`}>
      {status ?? "unknown"}
    </span>
  );
}

export default async function DashboardPage({
  searchParams,
}: DashboardPageProps) {
  const params = await searchParams;
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const [links, dashboard] = await Promise.all([
    getPublicLinks(),
    getDashboardMe(cookieHeader),
  ]);

  const state = getDashboardState({
    ...params,
    // Override URL params with actual API state: if the user has a session
    // with accessible guilds, don't show the "needs invite" banner just
    // because the redirect URL said so.
    ...(dashboard && dashboard.accessible_guilds.length > 0
      ? { status: "ready", needs_invite: undefined }
      : dashboard
        ? { status: "logged-in", needs_invite: "1" }
        : {}),
  });
  const accessibleGuilds = dashboard?.accessible_guilds ?? [];
  const selectedGuild =
    accessibleGuilds.find((guild) => guild.guild_id === params.guild_id) ??
    accessibleGuilds[0] ??
    null;
  const summary = selectedGuild
    ? await getGuildMessageSummary(selectedGuild.guild_id, cookieHeader)
    : null;
  const health = selectedGuild
    ? await getGuildHealthSummary(selectedGuild.guild_id, cookieHeader)
    : null;
  const funnel = selectedGuild
    ? await getGuildActivationFunnel(selectedGuild.guild_id, cookieHeader)
    : null;
  const retention = selectedGuild
    ? await getGuildRetentionCohorts(selectedGuild.guild_id, cookieHeader)
    : null;
  const pipeline = selectedGuild
    ? await getGuildPipelineHealth(selectedGuild.guild_id, cookieHeader)
    : null;
  const incidents = selectedGuild
    ? await getGuildPipelineIncidents(selectedGuild.guild_id, cookieHeader)
    : null;
  const replayHistory = selectedGuild
    ? await getGuildPipelineReplayHistory(selectedGuild.guild_id, cookieHeader)
    : null;
  const discardHistory = selectedGuild
    ? await getGuildPipelineDiscardHistory(selectedGuild.guild_id, cookieHeader)
    : null;
  const joinedStep = funnel?.steps[0]?.count ?? 0;

  return (
    <main className="flex min-h-screen flex-col bg-plum px-6 py-8 lg:px-8">
      <div className="mx-auto flex w-full max-w-[1280px] flex-1 flex-col gap-6">
        {dashboard && accessibleGuilds.length > 0 && (
          <DashboardNav guildId={selectedGuild?.guild_id} />
        )}

        {/* Hero / Status Banner */}
        <section className={`relative flex flex-col items-center justify-center text-center ${(!dashboard || accessibleGuilds.length === 0) ? 'flex-1' : 'py-12'}`}>
          <div className="relative z-10 flex flex-col items-center max-w-2xl">
            <h1 className="mt-6 text-3xl font-display font-semibold tracking-tight text-cream sm:text-4xl md:text-5xl">
              {state.title}
            </h1>
            <p className="mt-6 text-base leading-relaxed text-cream/70 sm:text-lg">
              {state.body}
            </p>
            {!dashboard || accessibleGuilds.length === 0 ? (
              <div className="mt-10 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-center">
                {!dashboard ? (
                  <a
                    href={links.login_url}
                    className="inline-flex h-12 items-center justify-center rounded-xl border border-border-light bg-surface px-8 text-base font-medium text-cream shadow-sm transition-all hover:bg-surface-light hover:text-white"
                  >
                    Log In
                  </a>
                ) : null}
                <a
                  href={links.invite_url}
                  className="inline-flex h-12 items-center justify-center rounded-xl bg-tan px-8 text-base font-medium text-plum shadow-sm transition-all hover:bg-sand"
                >
                  Invite To Server
                </a>
              </div>
            ) : null}
          </div>
          {/* Decorative background element */}
          <div className={`absolute left-1/2 top-1/2 -z-10 h-96 w-96 -translate-x-1/2 -translate-y-1/2 rounded-full blur-3xl opacity-20 ${state.bgClass.split(' ')[0]}`} />
        </section>

        {dashboard && accessibleGuilds.length > 0 ? (
          <div className="grid gap-6 lg:grid-cols-[260px_1fr] mt-8 w-full">
            <GuildSidebar
              accessibleGuilds={accessibleGuilds}
              basePath="/dashboard"
              dashboard={dashboard}
              selectedGuild={selectedGuild}
            />

            <div className="flex flex-col gap-6">
              {/* Key Metrics */}
              <div className="grid gap-4 sm:grid-cols-3">
                <div className="metric-card p-5">
                  <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                    Total Messages
                  </p>
                  <p className="mt-3 text-3xl font-semibold tracking-tight text-cream">
                    {summary ? summary.total_messages.toLocaleString("en-US") : "0"}
                  </p>
                  <p className="mt-1.5 text-xs text-cream/30">Last 7 days</p>
                </div>

                <div className="metric-card p-5">
                  <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                    Avg. Daily
                  </p>
                  <p className="mt-3 text-3xl font-semibold tracking-tight text-cream">
                    {summary && summary.daily.length > 0
                      ? Math.round(summary.total_messages / summary.daily.length).toLocaleString("en-US")
                      : "0"}
                  </p>
                  <p className="mt-1.5 text-xs text-cream/30">Messages per day</p>
                </div>

                <div className="metric-card p-5">
                  <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                    Status
                  </p>
                  <div className="mt-3 flex items-center gap-2.5">
                    <span className="relative flex h-2.5 w-2.5">
                      <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75"></span>
                      <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-emerald-400"></span>
                    </span>
                    <p className="text-lg font-semibold tracking-tight text-cream">
                      {summary?.backfill_status ? "Indexing" : "Live"}
                    </p>
                  </div>
                  <p className="mt-1.5 text-xs text-cream/30 truncate">
                    {summary?.backfill_status || "Unified tracking active"}
                  </p>
                </div>
              </div>

              {/* Chart */}
              <section className="card p-6">
                <div className="mb-6 flex items-end justify-between">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                      Activity Trend
                    </p>
                    <h3 className="text-lg font-semibold tracking-tight text-cream">
                      Message Volume
                    </h3>
                  </div>
                </div>

                {summary && summary.daily.length > 0 ? (
                  <MessageActivityChart data={summary.daily} />
                ) : (
                  <div className="flex h-[280px] items-center justify-center rounded-xl border border-dashed border-border-light text-sm text-cream/30">
                    No indexed messages are available for this 7-day window yet.
                  </div>
                )}
              </section>

              {/* Pipeline Health */}
              <section className="card p-6">
                <div className="mb-6 flex items-end justify-between gap-4">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                      Operations
                    </p>
                    <h3 className="text-lg font-semibold tracking-tight text-cream">
                      Pipeline Health
                    </h3>
                  </div>
                  <StatusBadge status={pipeline?.overall_status} />
                </div>

                <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                      Healthy Streams
                    </p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-cream">
                      {(pipeline?.healthy_streams ?? 0).toLocaleString("en-US")}
                      <span className="px-1 text-cream/20">/</span>
                      <span className="text-cream/50">
                        {(pipeline?.total_streams ?? 0).toLocaleString("en-US")}
                      </span>
                    </p>
                  </div>
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                      Ready + Pending
                    </p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-cream">
                      {(
                        (pipeline?.total_ready_messages ?? 0) +
                        (pipeline?.total_pending_messages ?? 0)
                      ).toLocaleString("en-US")}
                    </p>
                    <p className="mt-1 text-[11px] text-cream/30">
                      {(pipeline?.total_ready_messages ?? 0).toLocaleString("en-US")} ready,{" "}
                      {(pipeline?.total_pending_messages ?? 0).toLocaleString("en-US")} pending
                    </p>
                  </div>
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                      Scheduled Retries
                    </p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-cream">
                      {(pipeline?.total_scheduled_retry_messages ?? 0).toLocaleString("en-US")}
                    </p>
                    <p className="mt-1 text-[11px] text-cream/30">
                      Oldest overdue{" "}
                      {formatDuration(pipeline?.max_scheduled_retry_overdue_seconds ?? 0)}
                    </p>
                  </div>
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                      Dead Letters
                    </p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-tan">
                      {(pipeline?.total_dead_letter_messages ?? 0).toLocaleString("en-US")}
                    </p>
                    <p className="mt-1 text-[11px] text-cream/30">
                      Oldest ready age{" "}
                      {formatDuration(pipeline?.max_oldest_ready_age_seconds ?? 0)}
                    </p>
                  </div>
                </div>

                <div className="mt-5 overflow-x-auto">
                  <table className="dashboard-table w-full text-left text-sm">
                    <thead>
                      <tr>
                        <th>Stream</th>
                        <th className="text-right">Ready</th>
                        <th className="text-right">Pending</th>
                        <th className="text-right">Retries</th>
                        <th className="text-right">Dead letters</th>
                        <th className="text-right">Status</th>
                      </tr>
                    </thead>
                    <tbody>
                      {(pipeline?.streams ?? []).map((stream) => (
                        <tr key={stream.stream}>
                          <td>
                            <p className="text-cream/80 font-medium">{stream.label}</p>
                            <p className="text-[11px] text-cream/30 mt-0.5">
                              Oldest ready {formatDuration(stream.oldest_ready_age_seconds)}
                              {stream.scheduled_retry_overdue_seconds > 0
                                ? `, overdue ${formatDuration(stream.scheduled_retry_overdue_seconds)}`
                                : ""}
                              {stream.oldest_dead_letter_age_seconds > 0
                                ? `, dead letter ${formatDuration(stream.oldest_dead_letter_age_seconds)}`
                                : ""}
                            </p>
                          </td>
                          <td className="text-right font-mono text-cream/60">
                            {stream.ready_messages.toLocaleString("en-US")}
                          </td>
                          <td className="text-right font-mono text-cream/60">
                            {stream.pending_messages.toLocaleString("en-US")}
                          </td>
                          <td className="text-right font-mono text-cream/60">
                            {stream.scheduled_retry_messages.toLocaleString("en-US")}
                          </td>
                          <td className="text-right font-mono text-cream/60">
                            {stream.dead_letter_messages.toLocaleString("en-US")}
                          </td>
                          <td className="text-right">
                            <StatusBadge status={stream.status} />
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {(pipeline?.streams ?? []).length === 0 ? (
                    <div className="rounded-xl border border-dashed border-border-light px-4 py-8 text-center text-sm text-cream/30">
                      No pipeline streams are available yet.
                    </div>
                  ) : null}
                </div>
              </section>

              {/* Dead Letters */}
              <section className="card p-6">
                <div className="mb-6 flex items-end justify-between gap-4">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                      Failures
                    </p>
                    <h3 className="text-lg font-semibold tracking-tight text-cream">
                      Recent Dead Letters
                    </h3>
                  </div>
                  <p className="text-xs font-mono text-cream/30">
                    {(incidents?.total_dead_letter_messages ?? 0).toLocaleString("en-US")} total
                  </p>
                </div>

                {(incidents?.incidents ?? []).length > 0 ? (
                  <div className="grid gap-3">
                    {(incidents?.incidents ?? []).map((incident) => (
                      <div
                        key={`${incident.source_stream}-${incident.delivery_id}-${incident.failed_at}`}
                        className="card-inner p-4"
                      >
                        <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="inline-flex items-center rounded-md border border-red-500/20 bg-red-500/10 px-2 py-0.5 text-[11px] font-semibold text-red-400">
                                {incident.source_stream_label}
                              </span>
                              <span className="text-[11px] text-cream/30">
                                Attempt {incident.attempts}
                              </span>
                              <span className="text-[11px] text-cream/30">
                                {formatDuration(incident.age_seconds)} ago
                              </span>
                            </div>
                            <p className="mt-3 text-sm leading-relaxed text-cream/70">
                              {incident.error}
                            </p>
                            <p className="mt-3 rounded-lg border border-border bg-plum/40 px-3 py-2 font-mono text-[11px] leading-relaxed text-cream/40">
                              {incident.payload_preview}
                            </p>
                          </div>
                          <div className="shrink-0 text-right">
                            <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                              Delivery
                            </p>
                            <p className="mt-1.5 font-mono text-xs text-cream/40">
                              {incident.delivery_id}
                            </p>
                            {selectedGuild ? (
                              <div className="mt-3">
                                <DeadLetterActions
                                  deadLetterEntryId={incident.dead_letter_entry_id}
                                  guildId={selectedGuild.guild_id}
                                  sourceStream={incident.source_stream}
                                />
                              </div>
                            ) : null}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="rounded-xl border border-dashed border-border-light px-4 py-8 text-center text-sm text-cream/30">
                    No dead-lettered deliveries are waiting right now.
                  </div>
                )}
              </section>

              {/* Replay History */}
              <section className="card p-6">
                <div className="mb-6 flex items-end justify-between gap-4">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                      Audit
                    </p>
                    <h3 className="text-lg font-semibold tracking-tight text-cream">
                      Replay History
                    </h3>
                  </div>
                </div>

                {(replayHistory?.replays ?? []).length > 0 ? (
                  <div className="overflow-x-auto">
                    <table className="dashboard-table w-full text-left text-sm">
                      <thead>
                        <tr>
                          <th>When</th>
                          <th>User</th>
                          <th>Stream</th>
                          <th className="text-right">Attempts</th>
                          <th className="text-right">Delivery</th>
                        </tr>
                      </thead>
                      <tbody>
                        {(replayHistory?.replays ?? []).map((replay) => (
                          <tr key={`${replay.delivery_id}-${replay.replayed_at}`}>
                            <td className="text-cream/60">
                              {formatTimestamp(replay.replayed_at)}
                            </td>
                            <td className="text-cream/80">
                              <p className="font-medium">{replay.replayed_by_label}</p>
                              {replay.operator_reason ? (
                                <p className="mt-0.5 text-[11px] text-cream/30">
                                  {replay.operator_reason}
                                </p>
                              ) : null}
                            </td>
                            <td className="text-cream/60">
                              {replay.source_stream_label}
                            </td>
                            <td className="text-right font-mono text-cream/60">
                              {replay.attempts.toLocaleString("en-US")}
                            </td>
                            <td className="text-right font-mono text-cream/40">
                              {replay.delivery_id}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ) : (
                  <div className="rounded-xl border border-dashed border-border-light px-4 py-8 text-center text-sm text-cream/30">
                    No replay actions have been recorded yet.
                  </div>
                )}
              </section>

              {/* Discard History */}
              <section className="card p-6">
                <div className="mb-6 flex items-end justify-between gap-4">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                      Audit
                    </p>
                    <h3 className="text-lg font-semibold tracking-tight text-cream">
                      Discard History
                    </h3>
                  </div>
                </div>

                {(discardHistory?.discards ?? []).length > 0 ? (
                  <div className="overflow-x-auto">
                    <table className="dashboard-table w-full text-left text-sm">
                      <thead>
                        <tr>
                          <th>When</th>
                          <th>User</th>
                          <th>Stream</th>
                          <th className="text-right">Attempts</th>
                          <th className="text-right">Delivery</th>
                        </tr>
                      </thead>
                      <tbody>
                        {(discardHistory?.discards ?? []).map((discard) => (
                          <tr key={`${discard.delivery_id}-${discard.discarded_at}`}>
                            <td className="text-cream/60">
                              {formatTimestamp(discard.discarded_at)}
                            </td>
                            <td className="text-cream/80">
                              <p className="font-medium">{discard.discarded_by_label}</p>
                              {discard.operator_reason ? (
                                <p className="mt-0.5 text-[11px] text-cream/30">
                                  {discard.operator_reason}
                                </p>
                              ) : null}
                            </td>
                            <td className="text-cream/60">
                              {discard.source_stream_label}
                            </td>
                            <td className="text-right font-mono text-cream/60">
                              {discard.attempts.toLocaleString("en-US")}
                            </td>
                            <td className="text-right font-mono text-cream/40">
                              {discard.delivery_id}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ) : (
                  <div className="rounded-xl border border-dashed border-border-light px-4 py-8 text-center text-sm text-cream/30">
                    No discard actions have been recorded yet.
                  </div>
                )}
              </section>

              {/* Audience Summary */}
              <section className="card p-6">
                <div className="mb-6 flex items-end justify-between">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                      Guild Health
                    </p>
                    <h3 className="text-lg font-semibold tracking-tight text-cream">
                      Audience Summary
                    </h3>
                  </div>
                  <p className="text-xs font-mono text-cream/30">
                    Last {health?.days_requested ?? 30} days
                  </p>
                </div>

                <div className="grid gap-3 md:grid-cols-3 xl:grid-cols-6">
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">DAU</p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-tan">
                      {health?.dau.toLocaleString("en-US") ?? "0"}
                    </p>
                  </div>
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">WAU</p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-cream">
                      {health?.wau.toLocaleString("en-US") ?? "0"}
                    </p>
                  </div>
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">MAU</p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-cream">
                      {health?.mau.toLocaleString("en-US") ?? "0"}
                    </p>
                  </div>
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">Joins / Leaves</p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-cream">
                      {(health?.joined_members ?? 0).toLocaleString("en-US")}
                      <span className="px-1 text-cream/20">/</span>
                      <span className="text-cream/50">
                        {(health?.left_members ?? 0).toLocaleString("en-US")}
                      </span>
                    </p>
                  </div>
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">Join/Leave Ratio</p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-cream">
                      {health?.join_leave_ratio != null
                        ? `${health.join_leave_ratio.toFixed(1)}x`
                        : "n/a"}
                    </p>
                  </div>
                  <div className="card-inner p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">Onboarded</p>
                    <p className="mt-2.5 text-2xl font-semibold tracking-tight text-tan">
                      {health?.onboarding_completion_rate != null
                        ? `${Math.round(health.onboarding_completion_rate * 100)}%`
                        : "n/a"}
                    </p>
                    <p className="mt-1 text-[11px] text-cream/30">
                      {(health?.onboarded_members ?? 0).toLocaleString("en-US")} members
                    </p>
                  </div>
                </div>
              </section>

              {/* Activation Funnel */}
              <section className="card p-6">
                <div className="mb-6 flex items-end justify-between">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                      Onboarding
                    </p>
                    <h3 className="text-lg font-semibold tracking-tight text-cream">
                      Activation Funnel
                    </h3>
                  </div>
                  <p className="text-xs font-mono text-cream/30">
                    Last {funnel?.days_requested ?? 30} days
                  </p>
                </div>

                <div className="grid gap-3 md:grid-cols-3">
                  {(funnel?.steps ?? []).map((step) => {
                    const conversion = joinedStep > 0 ? (step.count / joinedStep) * 100 : 0;

                    return (
                      <div key={step.key} className="card-inner p-4">
                        <div className="flex items-end justify-between gap-3">
                          <div>
                            <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                              {step.label}
                            </p>
                            <p className="mt-2.5 text-2xl font-semibold tracking-tight text-cream">
                              {step.count.toLocaleString("en-US")}
                            </p>
                          </div>
                          <p className="text-xs font-mono text-cream/30">
                            {conversion.toFixed(0)}%
                          </p>
                        </div>
                        <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-surface">
                          <div
                            className="h-full rounded-full bg-tan/70 transition-all"
                            style={{ width: `${Math.min(conversion, 100)}%` }}
                          />
                        </div>
                      </div>
                    );
                  })}
                </div>
              </section>

              {/* Cohort Retention */}
              <section className="card p-6">
                <div className="mb-6 flex items-end justify-between">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                      Retention
                    </p>
                    <h3 className="text-lg font-semibold tracking-tight text-cream">
                      Cohort Retention
                    </h3>
                  </div>
                  <p className="text-xs font-mono text-cream/30">
                    Last {retention?.days_requested ?? 30} join cohorts
                  </p>
                </div>

                <div className="grid gap-4 md:grid-cols-2 mb-5">
                  <div className="card-inner p-5">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                      D7 Retention
                    </p>
                    <p className="mt-2.5 text-3xl font-semibold tracking-tight text-tan">
                      {retention?.d7_retention_rate != null
                        ? `${Math.round(retention.d7_retention_rate * 100)}%`
                        : "n/a"}
                    </p>
                    <p className="mt-1.5 text-xs text-cream/30">
                      Weighted across mature cohorts
                    </p>
                  </div>

                  <div className="card-inner p-5">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                      D30 Retention
                    </p>
                    <p className="mt-2.5 text-3xl font-semibold tracking-tight text-cream">
                      {retention?.d30_retention_rate != null
                        ? `${Math.round(retention.d30_retention_rate * 100)}%`
                        : "n/a"}
                    </p>
                    <p className="mt-1.5 text-xs text-cream/30">
                      Weighted across mature cohorts
                    </p>
                  </div>
                </div>

                <div className="overflow-x-auto">
                  <table className="dashboard-table w-full text-left text-sm">
                    <thead>
                      <tr>
                        <th>Cohort</th>
                        <th className="text-right">Joined</th>
                        <th className="text-right">D7</th>
                        <th className="text-right">D30</th>
                      </tr>
                    </thead>
                    <tbody>
                      {(retention?.cohorts ?? []).slice(0, 8).map((cohort) => (
                        <tr key={cohort.cohort_date}>
                          <td>
                            <p className="text-cream/80 font-medium">{cohort.cohort_date}</p>
                            <p className="text-[11px] text-cream/30 mt-0.5">
                              {cohort.cohort_age_days} days old
                            </p>
                          </td>
                          <td className="text-right font-mono text-cream/60">
                            {cohort.joined_count.toLocaleString("en-US")}
                          </td>
                          <td className="text-right">
                            <span className="font-mono text-cream/60">
                              {cohort.d7_retention_rate != null
                                ? `${Math.round(cohort.d7_retention_rate * 100)}%`
                                : "n/a"}
                            </span>
                            <p className="text-[11px] text-cream/30">
                              {cohort.d7_retained.toLocaleString("en-US")} retained
                            </p>
                          </td>
                          <td className="text-right">
                            <span className="font-mono text-cream/60">
                              {cohort.d30_retention_rate != null
                                ? `${Math.round(cohort.d30_retention_rate * 100)}%`
                                : "n/a"}
                            </span>
                            <p className="text-[11px] text-cream/30">
                              {cohort.d30_retained.toLocaleString("en-US")} retained
                            </p>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </section>
            </div>
          </div>
        ) : null}
      </div>
    </main>
  );
}
