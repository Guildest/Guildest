"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { AppealItem } from "@/lib/types";
import { approveAppeal, blockAppeal, deleteAppeal, summarizeAppeal } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";

interface AppealsTableProps {
  guildId: string;
  appeals: AppealItem[];
  canSummarize: boolean;
}

function formatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function statusClasses(status: string) {
  const normalized = status.toLowerCase();
  if (normalized === "approved") return "bg-green-500/10 text-green-400";
  if (normalized === "denied" || normalized === "deleted") return "bg-muted text-muted-foreground";
  return "bg-primary/10 text-primary";
}

export function AppealsTable({ guildId, appeals, canSummarize }: AppealsTableProps) {
  const router = useRouter();
  const [busy, setBusy] = useState<string | null>(null);

  const runAction = async (key: string, action: () => Promise<unknown>) => {
    setBusy(key);
    try {
      await action();
      router.refresh();
    } catch (error) {
      const message = error instanceof Error ? error.message : "Action failed.";
      window.alert(message);
    } finally {
      setBusy(null);
    }
  };

  if (appeals.length === 0) {
    return (
      <div className="rounded-2xl border border-dashed bg-muted/20 p-8 text-center text-muted-foreground">
        No appeals yet.
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {appeals.map((appeal) => {
        const status = appeal.status || "open";
        const isBusy = busy?.includes(appeal.id);
        const displayName = appeal.user_name || `User ${appeal.user_id}`;
        const avatarUrl = appeal.user_avatar
          ? `https://cdn.discordapp.com/avatars/${appeal.user_id}/${appeal.user_avatar}.png?size=96`
          : null;
        return (
          <Card key={appeal.id} className="border bg-card/70">
            <CardHeader className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
              <div className="flex items-center gap-3">
                {avatarUrl ? (
                  <img src={avatarUrl} alt={displayName} className="h-10 w-10 rounded-full border" />
                ) : (
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-secondary text-sm font-semibold text-secondary-foreground">
                    {displayName.charAt(0).toUpperCase()}
                  </div>
                )}
                <div>
                  <CardTitle className="text-base">{displayName}</CardTitle>
                  <p className="text-xs text-muted-foreground">Submitted {formatDate(appeal.created_at)}</p>
                </div>
              </div>
              <span className={`rounded-full px-3 py-1 text-xs font-medium ${statusClasses(status)}`}>
                {status.toUpperCase()}
              </span>
            </CardHeader>
            <CardContent className="space-y-3">
              <div>
                <p className="text-xs uppercase text-muted-foreground">Ban reason</p>
                <p className="text-sm">{appeal.ban_reason || "Not provided"}</p>
              </div>
              <div>
                <p className="text-xs uppercase text-muted-foreground">Appeal</p>
                <p className="text-sm whitespace-pre-wrap">{appeal.appeal_text}</p>
              </div>
              <div>
                <p className="text-xs uppercase text-muted-foreground">LLM summary</p>
                <p className="text-sm whitespace-pre-wrap">{appeal.summary || "Not generated yet."}</p>
              </div>
            </CardContent>
            <CardFooter className="flex flex-wrap gap-2 justify-end">
              <Button
                variant="outline"
                size="sm"
                disabled={!canSummarize || isBusy}
                onClick={() => runAction(`summarize-${appeal.id}`, () => summarizeAppeal(guildId, appeal.id))}
              >
                {appeal.summary ? "Refresh Summary" : "Summarize"}
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={status !== "open" || isBusy}
                onClick={() => {
                  if (!window.confirm("Unban this user and approve the appeal?")) return;
                  runAction(`unban-${appeal.id}`, () => approveAppeal(guildId, appeal.id));
                }}
              >
                Unban
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={isBusy}
                onClick={() => {
                  if (!window.confirm("Delete this appeal?")) return;
                  runAction(`delete-${appeal.id}`, () => deleteAppeal(guildId, appeal.id));
                }}
              >
                Delete
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={isBusy}
                onClick={() => {
                  if (!window.confirm("Block this user from future appeals?")) return;
                  runAction(`block-${appeal.id}`, () => blockAppeal(guildId, appeal.id));
                }}
              >
                Ban from Appeals
              </Button>
            </CardFooter>
          </Card>
        );
      })}
    </div>
  );
}
