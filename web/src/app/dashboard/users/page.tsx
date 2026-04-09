import { cookies } from "next/headers";
import { DashboardNav } from "@/components/dashboard-nav";
import { GuildSidebar } from "@/components/guild-sidebar";

import {
  getDashboardMe,
  getGuildUsersSummary,
} from "@/lib/public-api";

type UsersPageProps = {
  searchParams: Promise<{
    guild_id?: string;
  }>;
};

export default async function UsersPage({ searchParams }: UsersPageProps) {
  const params = await searchParams;
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const dashboard = await getDashboardMe(cookieHeader);

  const accessibleGuilds = dashboard?.accessible_guilds ?? [];
  const selectedGuild =
    accessibleGuilds.find((guild) => guild.guild_id === params.guild_id) ??
    accessibleGuilds[0] ??
    null;
  const usersSummary = selectedGuild
    ? await getGuildUsersSummary(selectedGuild.guild_id, cookieHeader)
    : null;
  const totalVoiceHours = (usersSummary?.total_voice_seconds ?? 0) / 3600;

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
              basePath="/dashboard/users"
              dashboard={dashboard}
              selectedGuild={selectedGuild}
            />
          )}

          <div className="flex flex-col gap-6">
            {/* Metric Cards */}
            <div className="grid gap-4 sm:grid-cols-3">
              <div className="metric-card p-5">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                  Total Active Users
                </p>
                <p className="mt-3 text-3xl font-semibold tracking-tight text-tan">
                  {usersSummary?.total_active_users.toLocaleString() ?? "0"}
                </p>
                <p className="mt-1.5 text-xs text-cream/30">
                  In the last {usersSummary?.days_requested ?? 7} days
                </p>
              </div>

              <div className="metric-card p-5">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                  Messages per User
                </p>
                <p className="mt-3 text-3xl font-semibold tracking-tight text-cream">
                  {(usersSummary?.avg_messages_per_active_user ?? 0).toFixed(1)}
                </p>
                <p className="mt-1.5 text-xs text-cream/30">Average messages per active user</p>
              </div>

              <div className="metric-card p-5">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
                  Voice Hours
                </p>
                <p className="mt-3 text-3xl font-semibold tracking-tight text-cream">
                  {totalVoiceHours.toFixed(1)}
                </p>
                <p className="mt-1.5 text-xs text-cream/30">Accumulated member voice time</p>
              </div>
            </div>

            {/* Leaderboard */}
            <section className="card p-6">
              <div className="mb-6">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-1">
                  Leaderboard
                </p>
                <h3 className="text-lg font-semibold tracking-tight text-cream">
                  Top Contributors
                </h3>
              </div>

              <div className="overflow-x-auto">
                <table className="dashboard-table w-full text-left text-sm">
                  <thead>
                    <tr>
                      <th>User</th>
                      <th className="text-right">Messages</th>
                      <th className="text-center">Reactions</th>
                      <th className="text-center">Voice</th>
                      <th className="text-right">Active Days</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(usersSummary?.users ?? []).map((user, i) => (
                      <tr key={user.discord_user_id}>
                        <td>
                          <div className="flex items-center gap-3">
                            <span
                              className={`flex h-7 w-7 items-center justify-center rounded-lg text-[11px] font-semibold ${
                                i === 0
                                  ? "bg-tan/20 text-tan"
                                  : i === 1
                                    ? "bg-sand/10 text-sand/70"
                                    : i === 2
                                      ? "bg-brown/20 text-brown"
                                      : "bg-surface-light text-cream/30"
                              }`}
                            >
                              {i + 1}
                            </span>
                            <div>
                              <span className="font-medium text-cream/80">{user.label}</span>
                              {user.secondary_label ? (
                                <p className="text-[11px] text-cream/30">{user.secondary_label}</p>
                              ) : null}
                            </div>
                          </div>
                        </td>
                        <td className="text-right font-mono text-cream/60">
                          {user.messages_sent.toLocaleString()}
                        </td>
                        <td className="text-center text-cream/40">
                          {user.reactions_added.toLocaleString()}
                        </td>
                        <td className="text-center text-cream/40">
                          {(user.voice_seconds / 3600).toFixed(1)}h
                        </td>
                        <td className="text-right text-cream/50 font-medium">
                          {user.active_days}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          </div>
        </div>
      </div>
    </main>
  );
}
