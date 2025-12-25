import Link from "next/link";
import { redirect } from "next/navigation";
import { backendFetch } from "@/lib/backend.server";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ArrowUpRight, Crown, ShieldCheck, Stars, Users2 } from "lucide-react";
import { MeResponse } from "@/lib/types";
import { ConnectGuildButton } from "@/components/connect-guild-button";
import { DisconnectGuildButton } from "@/components/disconnect-guild-button";

async function getMe(): Promise<MeResponse | null> {
  try {
    const res = await backendFetch("/me");
    if (res.status === 401) return null;
    if (!res.ok) throw new Error(`Failed to load /me (${res.status})`);
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

export default async function DashboardPage() {
  const me = await getMe();

  if (!me) {
    redirect("/");
  }

  const displayName = me.username || me.user_id;
  const avatarUrl = me.avatar
    ? `https://cdn.discordapp.com/avatars/${me.user_id}/${me.avatar}.png?size=128`
    : null;
  const planLabel = (me.plan || "free").toString().toUpperCase();

  const planLimit = (plan: string) => {
    const normalized = plan.toLowerCase();
    if (normalized === "premium") return 10;
    if (normalized === "plus") return 3;
    return 1;
  };

  const connectedLimit = me.connected_limit ?? planLimit(me.plan || "free");

  const connectedGuilds = me.guilds.filter((g) => g.connected);
  const availableGuilds = me.guilds.filter((g) => !g.connected && (g.permissions & 0x8 || g.is_owner));
  const applicationId = me.discord_client_id ?? null;
  const limitReached = connectedGuilds.length >= connectedLimit;

  return (
    <div className="space-y-10 max-w-6xl mx-auto">
      <section className="relative overflow-hidden rounded-2xl border bg-card/80 p-8">
        <div className="absolute inset-0 bg-gradient-to-br from-primary/10 via-transparent to-secondary/10" />
        <div className="relative z-10 space-y-6">
          <div className="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
            <div className="space-y-2">
              <p className="inline-flex items-center gap-2 text-xs uppercase tracking-[0.2em] text-muted-foreground">
                <Stars className="h-4 w-4 text-primary" /> Command Center
              </p>
              <div className="flex items-center gap-3">
                {avatarUrl ? (
                  <img
                    src={avatarUrl}
                    alt={displayName}
                    className="h-10 w-10 rounded-full border border-primary/40"
                  />
                ) : (
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-secondary text-sm font-semibold text-secondary-foreground">
                    {displayName.charAt(0).toUpperCase()}
                  </div>
                )}
                <h1 className="text-3xl font-semibold tracking-tight">
                  Welcome back, <span className="text-primary">{displayName}</span>
                </h1>
              </div>
              <p className="text-sm text-muted-foreground max-w-xl">
                Manage your servers, invite the bot, and jump into analytics in one place.
              </p>
            </div>
            <div className="grid gap-3 sm:grid-cols-3">
              <Card className="border-0 bg-background/40 p-4">
                <p className="text-xs text-muted-foreground">Connected</p>
                <p className="text-2xl font-semibold">
                  {connectedGuilds.length} <span className="text-xs text-muted-foreground">/ {connectedLimit}</span>
                </p>
              </Card>
              <Card className="border-0 bg-background/40 p-4">
                <p className="text-xs text-muted-foreground">Available</p>
                <p className="text-2xl font-semibold">{availableGuilds.length}</p>
              </Card>
              <Card className="border-0 bg-background/40 p-4">
                <p className="text-xs text-muted-foreground">Plan</p>
                <p className="text-2xl font-semibold">{planLabel}</p>
              </Card>
            </div>
          </div>

          <div className="grid gap-4 md:grid-cols-3">
            <div className="rounded-xl border bg-background/30 p-4">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Users2 className="h-4 w-4 text-secondary" />
                Connect your guilds
              </div>
              <p className="text-sm text-muted-foreground">
                Invite the bot before connecting to unlock features.
              </p>
            </div>
            <div className="rounded-xl border bg-background/30 p-4">
              <div className="flex items-center gap-2 text-sm font-medium">
                <ShieldCheck className="h-4 w-4 text-secondary" />
                Configure moderation
              </div>
              <p className="text-sm text-muted-foreground">
                Tune analytics and moderation per guild in settings.
              </p>
            </div>
            <div className="rounded-xl border bg-background/30 p-4">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Crown className="h-4 w-4 text-secondary" />
                Upgrade when ready
              </div>
              <p className="text-sm text-muted-foreground">
                Plus and Premium unlock deeper insights and reports.
              </p>
            </div>
          </div>
        </div>
      </section>

      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold">Connected Guilds</h2>
          {connectedGuilds.length > 0 && (
            <p className="text-sm text-muted-foreground">Jump back into an active dashboard.</p>
          )}
        </div>
        {connectedGuilds.length > 0 ? (
          <div className="grid gap-4 md:grid-cols-2">
            {connectedGuilds.map((guild) => (
              <div
                key={guild.guild_id}
                className="rounded-2xl border bg-card/70 p-4 shadow-sm"
              >
                <div className="flex items-center gap-4">
                  {guild.icon ? (
                    <img
                      src={`https://cdn.discordapp.com/icons/${guild.guild_id}/${guild.icon}.png`}
                      alt={guild.name || "Guild Icon"}
                      className="h-12 w-12 rounded-2xl"
                    />
                  ) : (
                    <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-secondary text-lg font-bold text-secondary-foreground">
                      {(guild.name || "?").charAt(0)}
                    </div>
                  )}
                  <div className="flex-1 min-w-0">
                    <p className="truncate text-base font-semibold">{guild.name}</p>
                    <p className="text-xs text-muted-foreground truncate">ID: {guild.guild_id}</p>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="rounded-full bg-secondary/20 px-2 py-1 text-xs text-secondary">
                      Connected
                    </span>
                    <Link href={`/dashboard/${guild.guild_id}`}>
                      <Button size="sm" className="gap-2">
                        Open
                        <ArrowUpRight className="h-4 w-4" />
                      </Button>
                    </Link>
                    <DisconnectGuildButton guildId={guild.guild_id} guildName={guild.name} />
                  </div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-2xl border border-dashed bg-muted/20 p-8 text-center text-muted-foreground">
            No connected guilds yet. Invite the bot to get started.
          </div>
        )}
      </section>

      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold">Ready to Connect</h2>
          <p className="text-sm text-muted-foreground">
            Only guilds where you are owner or admin are eligible.
          </p>
        </div>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {availableGuilds.map((guild) => {
            const botMissing = guild.bot_present === false;
            return (
              <div
                key={guild.guild_id}
                className="rounded-2xl border bg-card/70 p-4"
              >
                <div className="flex items-center gap-3">
                  {guild.icon ? (
                    <img
                      src={`https://cdn.discordapp.com/icons/${guild.guild_id}/${guild.icon}.png`}
                      alt={guild.name || "Guild Icon"}
                      className="h-12 w-12 rounded-2xl"
                    />
                  ) : (
                    <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-secondary text-lg font-bold text-secondary-foreground">
                      {(guild.name || "?").charAt(0)}
                    </div>
                  )}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-base font-semibold">{guild.name}</p>
                    <p className="text-xs text-muted-foreground truncate">ID: {guild.guild_id}</p>
                  </div>
                </div>
                <div className="mt-4 flex items-center justify-between">
                  <span className="rounded-full bg-primary/10 px-3 py-1 text-xs text-primary">
                    {guild.is_owner ? "Owner" : "Admin"}
                  </span>
                  <div className="text-xs text-muted-foreground">
                    {botMissing ? "Bot missing" : "Invite required"}
                  </div>
                </div>
                <div className="mt-4">
                  <ConnectGuildButton
                    guildId={guild.guild_id}
                    guildName={guild.name}
                    applicationId={applicationId}
                    botPresent={guild.bot_present ?? undefined}
                    disabled={limitReached}
                    disabledReason="Plan limit reached"
                  />
                </div>
              </div>
            );
          })}
          {availableGuilds.length === 0 && (
            <div className="col-span-full rounded-2xl border border-dashed bg-muted/20 p-8 text-center text-muted-foreground">
              No eligible guilds found right now.
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
