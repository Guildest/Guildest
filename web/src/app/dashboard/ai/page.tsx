import { cookies } from "next/headers";
import { DashboardNav } from "@/components/dashboard-nav";
import { GuildSidebar } from "@/components/guild-sidebar";
import {
  getDashboardMe,
  getAiSettings,
  getAiLivePulse,
  getPublicLinks,
  type AiGuildSettings,
  type AiLivePulse,
} from "@/lib/public-api";

type AiPageProps = {
  searchParams: Promise<{ guild_id?: string }>;
};

function StatCard({
  label,
  value,
  sub,
}: {
  label: string;
  value: number | string;
  sub?: string;
}) {
  return (
    <div className="rounded-2xl border border-border bg-surface p-5">
      <p className="text-xs font-medium text-cream/50 uppercase tracking-wide">{label}</p>
      <p className="mt-1 text-2xl font-semibold text-cream">{value}</p>
      {sub && <p className="mt-0.5 text-xs text-cream/40">{sub}</p>}
    </div>
  );
}

function SentimentBar({ pulse }: { pulse: AiLivePulse }) {
  const total =
    pulse.positive_sentiment_count +
    pulse.negative_sentiment_count +
    pulse.neutral_sentiment_count;

  if (total === 0) {
    return (
      <div className="h-2 w-full rounded-full bg-surface-light" />
    );
  }

  const posW = (pulse.positive_sentiment_count / total) * 100;
  const negW = (pulse.negative_sentiment_count / total) * 100;
  const neuW = (pulse.neutral_sentiment_count / total) * 100;

  return (
    <div className="flex h-2 w-full overflow-hidden rounded-full gap-0.5">
      {posW > 0 && (
        <div
          className="h-full rounded-full bg-emerald-500"
          style={{ width: `${posW}%` }}
          title={`Positive: ${pulse.positive_sentiment_count}`}
        />
      )}
      {neuW > 0 && (
        <div
          className="h-full rounded-full bg-tan/40"
          style={{ width: `${neuW}%` }}
          title={`Neutral: ${pulse.neutral_sentiment_count}`}
        />
      )}
      {negW > 0 && (
        <div
          className="h-full rounded-full bg-red-500"
          style={{ width: `${negW}%` }}
          title={`Negative: ${pulse.negative_sentiment_count}`}
        />
      )}
    </div>
  );
}

function LivePulsePanel({ pulse }: { pulse: AiLivePulse }) {
  const classified =
    pulse.total_observations > 0
      ? Math.round((pulse.classified_count / pulse.total_observations) * 100)
      : 0;

  return (
    <div className="rounded-2xl border border-border bg-surface p-6 space-y-6">
      <div className="flex items-start justify-between">
        <div>
          <h2 className="text-sm font-semibold text-cream">Live Pulse</h2>
          <p className="text-xs text-cream/40 mt-0.5">
            Last {pulse.window_minutes} minutes · {classified}% classified
          </p>
        </div>
        <span className="flex items-center gap-1.5 text-xs text-emerald-400 font-medium">
          <span className="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" />
          Live
        </span>
      </div>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <StatCard label="Messages" value={pulse.total_observations} />
        <StatCard label="Questions" value={pulse.question_count} />
        <StatCard label="Feedback" value={pulse.feedback_count} />
        <StatCard
          label="High Urgency"
          value={pulse.high_urgency_count}
          sub={pulse.high_urgency_count > 0 ? "needs attention" : "all clear"}
        />
      </div>

      <div className="space-y-2">
        <div className="flex justify-between text-xs text-cream/50">
          <span>Sentiment</span>
          <span>
            {pulse.positive_sentiment_count}+ / {pulse.neutral_sentiment_count}~ /{" "}
            {pulse.negative_sentiment_count}−
          </span>
        </div>
        <SentimentBar pulse={pulse} />
      </div>
    </div>
  );
}

function AiOffPanel({ guildId }: { guildId: string }) {
  return (
    <div className="rounded-2xl border border-border bg-surface p-8 text-center space-y-3">
      <p className="text-sm font-semibold text-cream">AI is off for this server</p>
      <p className="text-xs text-cream/50 max-w-sm mx-auto">
        Enable AI in Settings below to start capturing and classifying community activity. Content
        capture is disabled by default — you control which channels are monitored.
      </p>
      <a
        href={`/dashboard/ai?guild_id=${guildId}#settings`}
        className="inline-block mt-2 rounded-xl bg-tan/15 px-4 py-2 text-xs font-medium text-tan hover:bg-tan/25 transition-colors"
      >
        Go to Settings
      </a>
    </div>
  );
}

function SettingsPanel({
  settings,
  guildId,
}: {
  settings: AiGuildSettings;
  guildId: string;
}) {
  const rows: { label: string; desc: string; key: keyof AiGuildSettings }[] = [
    {
      label: "AI enabled",
      desc: "Master switch. Turns on observation, classification, and analysis for this server.",
      key: "ai_enabled",
    },
    {
      label: "Real-time alerts",
      desc: "Alert when multiple users hit the same issue or a high-urgency signal appears.",
      key: "real_time_alerts_enabled",
    },
    {
      label: "Daily briefing",
      desc: "Operational digest delivered once per day.",
      key: "daily_briefing_enabled",
    },
    {
      label: "Weekly report",
      desc: "Strategic trends report delivered once per week.",
      key: "weekly_report_enabled",
    },
    {
      label: "Owner DM",
      desc: "Deliver briefings and alerts via Discord DM to the server owner.",
      key: "owner_dm_enabled",
    },
  ];

  return (
    <div id="settings" className="rounded-2xl border border-border bg-surface p-6 space-y-1">
      <h2 className="text-sm font-semibold text-cream mb-4">Settings</h2>
      <form
        method="POST"
        action={`/api/dashboard/guilds/${guildId}/ai/settings`}
        className="space-y-0"
      >
        {rows.map(({ label, desc, key }) => (
          <div
            key={key}
            className="flex items-start justify-between gap-4 rounded-xl px-1 py-3 border-b border-border/50 last:border-0"
          >
            <div>
              <p className="text-sm text-cream font-medium">{label}</p>
              <p className="text-xs text-cream/40 mt-0.5">{desc}</p>
            </div>
            <div
              className={`mt-0.5 h-5 w-9 shrink-0 rounded-full transition-colors ${
                settings[key] ? "bg-tan" : "bg-surface-light"
              }`}
              title={settings[key] ? "On" : "Off"}
            />
          </div>
        ))}

        <div className="pt-4">
          <p className="text-xs text-cream/40">
            To change settings, use the API or connect a settings form component.
            Content capture per channel is controlled in channel settings.
          </p>
        </div>
      </form>
    </div>
  );
}

export default async function AiDashboardPage({ searchParams }: AiPageProps) {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore
    .getAll()
    .map((c) => `${c.name}=${c.value}`)
    .join("; ");

  const params = await searchParams;
  const [me, links] = await Promise.all([
    getDashboardMe(cookieHeader),
    getPublicLinks(),
  ]);

  const selectedGuildId =
    params.guild_id ?? me?.accessible_guilds?.[0]?.guild_id ?? null;

  const [settings, pulse] = selectedGuildId
    ? await Promise.all([
        getAiSettings(selectedGuildId, cookieHeader),
        getAiLivePulse(selectedGuildId, 60, cookieHeader),
      ])
    : [null, null];

  return (
    <div className="min-h-screen bg-background text-cream">
      <div className="mx-auto max-w-6xl px-4 py-8 space-y-6">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <DashboardNav guildId={selectedGuildId ?? undefined} />
        </div>

        <div className="grid grid-cols-1 gap-6 lg:grid-cols-[220px_1fr]">
          <GuildSidebar
            guilds={me?.accessible_guilds ?? []}
            selectedGuildId={selectedGuildId}
            installUrl={links?.install_url ?? "#"}
          />

          <div className="space-y-4">
            {!selectedGuildId && (
              <div className="rounded-2xl border border-border bg-surface p-6 text-center text-sm text-cream/50">
                Select a server to view AI insights.
              </div>
            )}

            {selectedGuildId && settings && !settings.ai_enabled && (
              <AiOffPanel guildId={selectedGuildId} />
            )}

            {selectedGuildId && settings?.ai_enabled && pulse && (
              <LivePulsePanel pulse={pulse} />
            )}

            {selectedGuildId && settings?.ai_enabled && !pulse && (
              <div className="rounded-2xl border border-border bg-surface p-6 text-center text-sm text-cream/50">
                No observations yet. Activity will appear here once messages are captured and
                classified.
              </div>
            )}

            {selectedGuildId && settings && (
              <SettingsPanel settings={settings} guildId={selectedGuildId} />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
