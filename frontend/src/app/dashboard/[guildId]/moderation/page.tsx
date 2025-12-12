import { backendFetch } from "@/lib/backend.server";
import { ModerationLogsResponse } from "@/lib/types";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatDate } from "@/lib/utils";
import { AlertTriangle, Ban, MessageSquareX, Shield } from "lucide-react";

async function getModerationLogs(guildId: string): Promise<ModerationLogsResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/moderation/logs`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

function getActionIcon(action: string | null) {
  switch (action?.toLowerCase()) {
    case "ban":
      return <Ban className="h-4 w-4 text-destructive" />;
    case "kick":
      return <UserX className="h-4 w-4 text-orange-500" />;
    case "warn":
      return <AlertTriangle className="h-4 w-4 text-yellow-500" />;
    case "delete":
      return <MessageSquareX className="h-4 w-4 text-blue-500" />;
    default:
      return <Shield className="h-4 w-4 text-muted-foreground" />;
  }
}

// Missing UserX import, so defining a fallback or adding it to imports
import { UserX } from "lucide-react";

export default async function ModerationPage({
  params,
}: {
  params: Promise<{ guildId: string }>;
}) {
  const { guildId } = await params;
  const logs = await getModerationLogs(guildId);

  return (
    <div className="space-y-8 max-w-6xl mx-auto">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Moderation Logs</h1>
        <p className="text-muted-foreground">
          Recent automated and manual moderation actions.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Audit Log</CardTitle>
        </CardHeader>
        <CardContent>
          {!logs || logs.items.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              No moderation logs found.
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[100px]">Action</TableHead>
                  <TableHead>User</TableHead>
                  <TableHead>Reason</TableHead>
                  <TableHead className="text-right">Date</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {logs.items.map((log) => (
                  <TableRow key={log.id}>
                    <TableCell className="font-medium flex items-center gap-2">
                      {getActionIcon(log.action)}
                      <span className="capitalize">{log.action || "Unknown"}</span>
                    </TableCell>
                    <TableCell>{log.author_id || "System"}</TableCell>
                    <TableCell className="max-w-[300px] truncate" title={log.reason || ""}>
                        {log.reason || "No reason provided"}
                    </TableCell>
                    <TableCell className="text-right">
                      {new Date(log.created_at).toLocaleString()}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
