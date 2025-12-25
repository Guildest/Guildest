import { backendFetch } from "@/lib/backend.server";
import { MeResponse, ModerationLogsResponse } from "@/lib/types";
import { ModerationLogView } from "@/components/moderation-log-view";

async function getModerationLogs(guildId: string): Promise<ModerationLogsResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/moderation/logs?include_users=true`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

async function getMe(): Promise<MeResponse | null> {
  try {
    const res = await backendFetch("/me");
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

export default async function ModerationPage({
  params,
}: {
  params: Promise<{ guildId: string }>;
}) {
  const { guildId } = await params;
  const [logs, me] = await Promise.all([getModerationLogs(guildId), getMe()]);

  return (
    <div className="space-y-8 max-w-6xl mx-auto">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Moderation Logs</h1>
        <p className="text-muted-foreground">
          Review automated flags and moderator actions with filters, exports, and detail panels.
        </p>
      </div>

      {!logs ? (
        <div className="rounded-2xl border border-dashed bg-muted/20 p-8 text-center text-muted-foreground">
          Unable to load moderation logs. Confirm your plan and permissions.
        </div>
      ) : (
        <ModerationLogView
          guildId={guildId}
          initialLogs={logs.items}
          initialUsers={logs.users ?? {}}
          botId={me?.discord_client_id ?? null}
        />
      )}
    </div>
  );
}
