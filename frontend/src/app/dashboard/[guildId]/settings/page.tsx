import { backendFetch } from "@/lib/backend.server";
import { GuildSettings } from "@/lib/types";
import { SettingsForm } from "@/components/settings-form";

async function getSettings(guildId: string): Promise<GuildSettings | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/settings`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

export default async function SettingsPage({
  params,
}: {
  params: Promise<{ guildId: string }>;
}) {
  const { guildId } = await params;
  const settings = await getSettings(guildId);

  if (!settings) {
    return (
      <div className="p-8 text-center text-muted-foreground">
        Failed to load settings or you do not have permission to manage this guild.
      </div>
    );
  }

  return (
    <div className="space-y-8 max-w-4xl mx-auto">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
        <p className="text-muted-foreground">
          Manage configuration for this guild.
        </p>
      </div>

      <SettingsForm initialSettings={settings} guildId={guildId} />
    </div>
  );
}
