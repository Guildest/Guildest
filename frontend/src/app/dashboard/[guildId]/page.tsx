import { backendFetch } from "@/lib/backend.server";
import { DashboardOverview } from "@/lib/types";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Check, X, Shield, BarChart3, Settings } from "lucide-react";
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

  return (
    <div className="space-y-8 max-w-6xl mx-auto">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Overview</h1>
        <p className="text-muted-foreground">
          Guild ID: {overview.guild_id} • Plan: <span className="uppercase font-semibold">{overview.plan}</span>
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Link href={`/dashboard/${guildId}/analytics`}>
          <Card className="hover:border-primary/50 transition-colors cursor-pointer h-full">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Analytics</CardTitle>
              <BarChart3 className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">View Stats</div>
              <p className="text-xs text-muted-foreground">
                Message volume and activity
              </p>
            </CardContent>
          </Card>
        </Link>
        <Link href={`/dashboard/${guildId}/moderation`}>
          <Card className="hover:border-primary/50 transition-colors cursor-pointer h-full">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Moderation</CardTitle>
              <Shield className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">Logs & Actions</div>
              <p className="text-xs text-muted-foreground">
                View audit logs and warnings
              </p>
            </CardContent>
          </Card>
        </Link>
        <Link href={`/dashboard/${guildId}/settings`}>
          <Card className="hover:border-primary/50 transition-colors cursor-pointer h-full">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Settings</CardTitle>
              <Settings className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">Configure</div>
              <p className="text-xs text-muted-foreground">
                Bot settings and modules
              </p>
            </CardContent>
          </Card>
        </Link>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Feature Status</CardTitle>
          <CardDescription>
            Features enabled for this guild based on your plan.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            {Object.entries(overview.features).map(([key, enabled]) => (
              <div key={key} className="flex items-center justify-between border-b pb-4 last:border-0 last:pb-0">
                <div className="flex flex-col">
                  <span className="font-medium capitalize">
                    {key.replace(/_/g, " ")}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  {enabled ? (
                    <span className="flex items-center gap-1 text-sm text-secondary">
                      <Check className="h-4 w-4" /> Enabled
                    </span>
                  ) : (
                    <span className="flex items-center gap-1 text-sm text-muted-foreground">
                      <X className="h-4 w-4" /> Disabled
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
