"use client";

import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { CheckCircle2, ExternalLink, Loader2, Plus, X } from "lucide-react";
import { connectGuild } from "@/lib/api";
import { useRouter } from "next/navigation";

const DEFAULT_BOT_PERMISSIONS =
  1024 + // View Channels
  2048 + // Send Messages
  8192 + // Manage Messages
  16384 + // Embed Links
  32768 + // Attach Files
  65536 + // Read Message History
  262144 + // Use External Emojis
  64; // Add Reactions

function buildInviteUrl(clientId: string, guildId: string, permissions: number) {
  const params = new URLSearchParams({
    client_id: clientId,
    scope: "bot applications.commands",
    permissions: String(permissions),
    guild_id: guildId,
    disable_guild_select: "true",
  });
  return `https://discord.com/api/oauth2/authorize?${params.toString()}`;
}

interface ConnectGuildButtonProps {
  guildId: string;
  guildName?: string | null;
  applicationId?: string | null;
  permissions?: number;
  botPresent?: boolean | null;
  disabled?: boolean;
  disabledReason?: string;
}

export function ConnectGuildButton({
  guildId,
  guildName,
  applicationId,
  permissions = DEFAULT_BOT_PERMISSIONS,
  botPresent,
  disabled = false,
  disabledReason,
}: ConnectGuildButtonProps) {
  const [open, setOpen] = useState(false);
  const [invited, setInvited] = useState(Boolean(botPresent));
  const [loading, setLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const router = useRouter();

  const inviteUrl = useMemo(() => {
    if (!applicationId) return null;
    return buildInviteUrl(applicationId, guildId, permissions);
  }, [applicationId, guildId, permissions]);

  const handleInvite = () => {
    if (!inviteUrl) return;
    window.open(inviteUrl, "_blank", "noopener,noreferrer");
    setInvited(true);
  };

  const handleConnect = async () => {
    setLoading(true);
    setErrorMessage(null);
    try {
      await connectGuild(guildId);
      setOpen(false);
      router.refresh();
    } catch (error) {
      console.error("Failed to connect guild", error);
      const message = error instanceof Error ? error.message : "Failed to connect guild. Please try again.";
      setErrorMessage(message);
    } finally {
      setLoading(false);
    }
  };

  const resetDialog = () => {
    if (loading) return;
    setOpen(false);
    setInvited(Boolean(botPresent));
    setErrorMessage(null);
  };

  return (
    <>
      <Button
        className="w-full gap-2"
        onClick={() => setOpen(true)}
        disabled={disabled}
      >
        <Plus className="h-4 w-4" />
        {disabled ? (disabledReason ?? "Limit reached") : "Invite & Connect"}
      </Button>

      {open && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4"
          onClick={resetDialog}
          role="presentation"
        >
          <div
            className="w-full max-w-lg rounded-2xl border bg-card shadow-xl"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-label="Invite bot and connect guild"
          >
            <div className="flex items-center justify-between border-b px-6 py-4">
              <div>
                <p className="text-sm text-muted-foreground">Connect guild</p>
                <h2 className="text-lg font-semibold">{guildName || "Your server"}</h2>
              </div>
              <Button variant="ghost" size="icon" onClick={resetDialog} disabled={loading}>
                <X className="h-4 w-4" />
              </Button>
            </div>

            <div className="space-y-4 px-6 py-5">
              <div className="rounded-xl border border-dashed border-primary/50 bg-primary/5 p-4">
                <p className="text-sm text-muted-foreground">Step 1</p>
                <p className="font-medium">
                  {botPresent ? "Bot already in this guild." : "Invite the bot to this guild."}
                </p>
                <p className="text-sm text-muted-foreground">
                  {botPresent
                    ? "You can re-open the invite if needed."
                    : "Discord will open in a new tab so you can authorize the bot."}
                </p>
                <div className="mt-3">
                  {inviteUrl ? (
                    <Button
                      variant="outline"
                      className="gap-2"
                      onClick={handleInvite}
                    >
                      <ExternalLink className="h-4 w-4" />
                      {botPresent ? "Open Invite Again" : "Open Discord Invite"}
                    </Button>
                  ) : (
                    <p className="text-sm text-destructive">
                      Missing Discord application ID. Set it in the API env.
                    </p>
                  )}
                </div>
              </div>

              <div className="rounded-xl border p-4">
                <p className="text-sm text-muted-foreground">Step 2</p>
                <p className="font-medium">Mark the guild as connected.</p>
                <p className="text-sm text-muted-foreground">
                  After authorizing, click below to finish.
                </p>
                <div className="mt-3">
                  <Button
                    className="gap-2"
                    onClick={handleConnect}
                    disabled={!invited || loading}
                  >
                    {loading ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <CheckCircle2 className="h-4 w-4" />
                    )}
                    {loading ? "Connecting..." : "Finish Connection"}
                  </Button>
                </div>
              </div>

              {errorMessage && (
                <div className="rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
                  {errorMessage}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
