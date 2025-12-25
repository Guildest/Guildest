import Link from "next/link";
import { backendFetch } from "@/lib/backend.server";
import { AppealsResponse, DashboardOverview } from "@/lib/types";
import { AppealsTable } from "@/components/appeals-table";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

async function getAppeals(guildId: string): Promise<AppealsResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/appeals`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

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

export default async function AppealsPage({
  params,
}: {
  params: Promise<{ guildId: string }>;
}) {
  const { guildId } = await params;
  const [appeals, overview] = await Promise.all([getAppeals(guildId), getOverview(guildId)]);

  if (!appeals) {
    return <div>Failed to load appeals.</div>;
  }

  const plan = overview?.plan ?? "free";
  const canSummarize = plan === "plus" || plan === "premium";

  return (
    <div className="space-y-6 max-w-6xl mx-auto">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Appeals</h1>
          <p className="text-muted-foreground">
            Review ban appeals submitted via the bot's DM modal.
          </p>
        </div>
        {!canSummarize && (
          <Link href={`/dashboard/${guildId}/billing`}>
            <Button size="sm">Upgrade for AI summaries</Button>
          </Link>
        )}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Appeals Queue</CardTitle>
          <CardDescription>
            {canSummarize
              ? "AI summaries are enabled for this guild."
              : "Upgrade to Plus or Premium to unlock AI appeal summaries."}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <AppealsTable guildId={guildId} appeals={appeals.items} canSummarize={canSummarize} />
        </CardContent>
      </Card>
    </div>
  );
}
