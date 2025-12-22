export type GuildSummary = {
  guild_id: string;
  name: string | null;
  icon: string | null;
  is_owner: boolean;
  permissions: number;
  connected: boolean;
};

export type MeResponse = {
  user_id: string;
  plan: "free" | "plus" | "premium" | string;
  guilds: GuildSummary[];
};

export type DashboardOverview = {
  guild_id: string;
  plan: "free" | "plus" | "premium" | string;
  features: {
    moderation_logs: boolean;
    sentiment_reports: boolean;
    event_recommendations: boolean;
    analytics_extended: boolean;
  };
};

export type MessageCountPoint = { time_bucket: string; count: number };
export type MessageCountsResponse = { guild_id: string; from: string; to: string; points: MessageCountPoint[] };

export type SentimentPoint = { day: string; sentiment: string; score: number | null };
export type SentimentDailyResponse = { guild_id: string; from: string; to: string; points: SentimentPoint[] };

export type ModerationLogItem = {
  id: number;
  message_id: string | null;
  channel_id: string | null;
  author_id: string | null;
  action: string | null;
  reason: string | null;
  created_at: string;
};

export type ModerationLogsResponse = { guild_id: string; items: ModerationLogItem[] };

export type GuildSettings = {
  guild_id: string;
  prefix: string;
  language: string;
  timezone: string;
  analytics_enabled: boolean;
  sentiment_enabled: boolean;
  moderation_enabled: boolean;
  welcome_channel_id?: string;
  log_channel_id?: string;
};

