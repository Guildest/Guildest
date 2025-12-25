"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import { disconnectGuild } from "@/lib/api";

interface DisconnectGuildButtonProps {
  guildId: string;
  guildName?: string | null;
}

export function DisconnectGuildButton({ guildId, guildName }: DisconnectGuildButtonProps) {
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  const handleDisconnect = async () => {
    const label = guildName ? `"${guildName}"` : "this guild";
    if (!window.confirm(`Remove ${label} from your connected servers?`)) {
      return;
    }
    setLoading(true);
    try {
      await disconnectGuild(guildId);
      router.refresh();
    } catch (error) {
      console.error("Failed to disconnect guild", error);
      const message = error instanceof Error ? error.message : "Failed to disconnect guild.";
      window.alert(message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Button
      variant="outline"
      size="sm"
      onClick={handleDisconnect}
      disabled={loading}
    >
      {loading ? "Removing..." : "Remove"}
    </Button>
  );
}
