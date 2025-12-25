import { backendFetch } from "@/lib/backend.server";
import { GuildSettings } from "@/lib/types";
import { SettingsForm } from "@/components/settings-form";
import { Button } from "@/components/ui/button";
import { Settings, Info } from "lucide-react";

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

const SECTIONS = [
  { href: "#general", label: "General", icon: Settings },
  { href: "#features", label: "Features", icon: Info },
  { href: "#warnings", label: "Warnings", icon: Info },
  { href: "#channels", label: "Channels", icon: Info },
];

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
    <div className="space-y-6 max-w-7xl mx-auto">
      <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Guild Settings</h1>
          <p className="text-muted-foreground mt-1">
            Configure bot behavior and features for your server.
          </p>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-[280px_1fr]">
        <aside className="space-y-4 lg:sticky lg:top-6 lg:self-start">
          <div className="rounded-xl border bg-card/60 p-4">
            <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-3">
              Navigation
            </p>
            <nav className="flex flex-wrap gap-2 lg:flex-col lg:gap-1.5">
              {SECTIONS.map((section) => {
                const Icon = section.icon;
                return (
                  <a
                    key={section.href}
                    href={section.href}
                    className="flex items-center gap-2.5 rounded-lg border px-3 py-2 text-sm text-muted-foreground transition hover:border-primary/50 hover:bg-muted/50 hover:text-foreground"
                  >
                    <Icon className="h-4 w-4" />
                    {section.label}
                  </a>
                );
              })}
            </nav>
          </div>
          <div className="rounded-xl border border-dashed bg-muted/30 p-4 text-xs text-muted-foreground">
            <div className="flex items-start gap-2">
              <Info className="h-4 w-4 shrink-0 mt-0.5" />
              <p>Changes are saved automatically when you click the save button at the bottom of each section.</p>
            </div>
          </div>
        </aside>

        <SettingsForm initialSettings={settings} guildId={guildId} />
      </div>
    </div>
  );
}
