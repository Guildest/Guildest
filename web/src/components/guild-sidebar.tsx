import type { AccessibleGuild, DashboardMe } from "@/lib/public-api";

type GuildSidebarProps = {
  accessibleGuilds: AccessibleGuild[];
  basePath: string;
  dashboard: DashboardMe;
  selectedGuild: AccessibleGuild | null;
};

export function GuildSidebar({
  accessibleGuilds,
  basePath,
  dashboard,
  selectedGuild,
}: GuildSidebarProps) {
  return (
    <aside className="flex flex-col gap-5">
      <div className="card p-5">
        <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35 mb-4">
          Profile
        </p>
        <h2 className="text-lg font-semibold tracking-tight text-cream">
          {dashboard.user.display_name}
        </h2>
        <p className="mt-1 text-xs text-cream/40 font-mono">
          @{dashboard.user.username}
        </p>
      </div>

      <div className="card p-5">
        <div className="flex items-center justify-between mb-4">
          <p className="text-[11px] font-semibold uppercase tracking-wider text-cream/35">
            Servers
          </p>
          <span className="flex h-5 min-w-5 items-center justify-center rounded-full bg-surface-light px-1.5 text-[10px] font-mono text-cream/40">
            {accessibleGuilds.length}
          </span>
        </div>

        <div className="flex flex-col gap-1.5">
          {accessibleGuilds.map((guild) => {
            const isSelected = selectedGuild?.guild_id === guild.guild_id;

            return (
              <a
                key={guild.guild_id}
                href={`${basePath}?guild_id=${guild.guild_id}`}
                className={`group flex items-center justify-between rounded-xl px-3.5 py-2.5 text-sm transition-all ${
                  isSelected
                    ? "bg-tan/15 text-tan border border-tan/20"
                    : "text-cream/60 hover:bg-surface-light hover:text-cream border border-transparent"
                }`}
              >
                <span className="truncate pr-3 font-medium">{guild.guild_name}</span>
                <span
                  className={`shrink-0 text-[10px] font-mono ${
                    isSelected
                      ? "text-tan/60"
                      : "text-cream/25 group-hover:text-cream/40"
                  }`}
                >
                  {guild.member_count >= 1000
                    ? `${(guild.member_count / 1000).toFixed(1)}k`
                    : guild.member_count}
                </span>
              </a>
            );
          })}
        </div>
      </div>
    </aside>
  );
}
