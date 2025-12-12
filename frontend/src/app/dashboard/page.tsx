import Link from "next/link";
import { redirect } from "next/navigation";
import { backendFetch } from "@/lib/backend.server";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Plus, ExternalLink } from "lucide-react";
import { GuildSummary, MeResponse } from "@/lib/types";
import { ConnectGuildButton } from "@/components/connect-guild-button";

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

  const connectedGuilds = me.guilds.filter((g) => g.connected);
  const availableGuilds = me.guilds.filter((g) => !g.connected && (g.permissions & 0x8 || g.is_owner));

  return (
    <div className="space-y-8 max-w-6xl mx-auto">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p className="text-muted-foreground">
          Welcome back, <span className="font-medium text-foreground">{me.user_id}</span>
        </p>
      </div>

      {connectedGuilds.length > 0 && (
        <section className="space-y-4">
          <h2 className="text-xl font-semibold">Active Guilds</h2>
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {connectedGuilds.map((guild) => (
              <Link key={guild.guild_id} href={`/dashboard/${guild.guild_id}`}>
                <Card className="h-full hover:border-primary/50 transition-colors cursor-pointer">
                  <CardHeader className="flex flex-row items-center gap-4 pb-2">
                    {guild.icon ? (
                      <img
                        src={`https://cdn.discordapp.com/icons/${guild.guild_id}/${guild.icon}.png`}
                        alt={guild.name || "Guild Icon"}
                        className="h-12 w-12 rounded-full"
                      />
                    ) : (
                      <div className="flex h-12 w-12 items-center justify-center rounded-full bg-secondary text-lg font-bold">
                        {(guild.name || "?").charAt(0)}
                      </div>
                    )}
                    <div className="flex flex-col overflow-hidden">
                      <CardTitle className="text-base truncate">{guild.name}</CardTitle>
                      <CardDescription className="truncate">ID: {guild.guild_id}</CardDescription>
                    </div>
                  </CardHeader>
                  <CardContent>
                    <div className="flex items-center text-xs text-muted-foreground">
                      <span className="flex h-2 w-2 rounded-full bg-green-500 mr-2" />
                      Connected
                    </div>
                  </CardContent>
                </Card>
              </Link>
            ))}
          </div>
        </section>
      )}

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">Available Guilds</h2>
        <p className="text-sm text-muted-foreground">
          Guilds you own or have administrative permissions in that are not yet connected.
        </p>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {availableGuilds.map((guild) => (
            <Card key={guild.guild_id}>
              <CardHeader className="flex flex-row items-center gap-4 pb-2">
                {guild.icon ? (
                  <img
                    src={`https://cdn.discordapp.com/icons/${guild.guild_id}/${guild.icon}.png`}
                    alt={guild.name || "Guild Icon"}
                    className="h-12 w-12 rounded-full"
                  />
                ) : (
                  <div className="flex h-12 w-12 items-center justify-center rounded-full bg-secondary text-lg font-bold">
                    {(guild.name || "?").charAt(0)}
                  </div>
                )}
                <div className="flex flex-col overflow-hidden">
                  <CardTitle className="text-base truncate">{guild.name}</CardTitle>
                  <CardDescription className="truncate">ID: {guild.guild_id}</CardDescription>
                </div>
              </CardHeader>
              <CardFooter className="pt-4">
                <ConnectGuildButton guildId={guild.guild_id} />
              </CardFooter>
            </Card>
          ))}
          {availableGuilds.length === 0 && (
            <div className="col-span-full py-8 text-center text-muted-foreground bg-muted/20 rounded-lg border border-dashed">
              No new eligible guilds found.
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
