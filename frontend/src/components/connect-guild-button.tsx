"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Plus, Loader2 } from "lucide-react";
import { connectGuild } from "@/lib/api";
import { useRouter } from "next/navigation";

export function ConnectGuildButton({ guildId }: { guildId: string }) {
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  const handleConnect = async () => {
    setLoading(true);
    try {
      await connectGuild(guildId);
      router.refresh();
    } catch (error) {
      console.error("Failed to connect guild", error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Button className="w-full gap-2" onClick={handleConnect} disabled={loading}>
      {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
      Connect Guild
    </Button>
  );
}
