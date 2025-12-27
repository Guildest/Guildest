export type GuildSummary = {
  guild_id: string;
  name: string | null;
  icon: string | null;
  is_owner: boolean;
  permissions: number;
  connected: boolean;
  bot_present?: boolean | null;
};

export type MeResponse = {
  user_id: string;
  username?: string | null;
  avatar?: string | null;
  plan: "free" | "plus" | "premium" | string;
  connected_limit?: number | null;
  guilds: GuildSummary[];
  discord_client_id?: string | null;
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
  stats?: {
    messages_24h: number;
    messages_7d: number;
    messages_30d: number;
    moderation_actions_24h: number;
    moderation_actions_7d: number;
    sentiment_score: number | null;
    sentiment_label: string | null;
    sentiment_trend: "up" | "down" | "stable" | null;
  };
};

export type MessageCountPoint = { time_bucket: string; count: number };
export type MessageCountsResponse = { guild_id: string; from: string; to: string; points: MessageCountPoint[] };

export type SentimentPoint = { day: string; sentiment: string; score: number | null };
export type SentimentDailyResponse = { guild_id: string; from: string; to: string; points: SentimentPoint[] };

export type AnalyticsSummaryPoint = {
  day: string;
  messages: number;
  voice_minutes: number;
  active_channels: number;
  dau_est: number | null;
  wau_est: number | null;
  mau_est: number | null;
};

export type AnalyticsSummaryResponse = {
  guild_id: string;
  from: string;
  to: string;
  points: AnalyticsSummaryPoint[];
};

export type AnalyticsVoicePoint = {
  bucket_start: string;
  total_seconds: number;
  peak_concurrent: number;
};

export type AnalyticsVoiceResponse = {
  guild_id: string;
  from: string;
  to: string;
  bucket_size: number;
  points: AnalyticsVoicePoint[];
};

export type AnalyticsCommandPoint = {
  bucket_start: string;
  count: number;
};

export type AnalyticsCommandResponse = {
  guild_id: string;
  from: string;
  to: string;
  bucket_size: number;
  points: AnalyticsCommandPoint[];
};

export type AnalyticsTopChannel = { channel_id: string; count: number };
export type AnalyticsTopChannelsResponse = {
  guild_id: string;
  from: string;
  to: string;
  limit: number;
  points: AnalyticsTopChannel[];
};

export type AnalyticsTopCommand = { command_name: string; count: number };
export type AnalyticsTopCommandsResponse = {
  guild_id: string;
  from: string;
  to: string;
  limit: number;
  points: AnalyticsTopCommand[];
};

export type AnalyticsMessageBucketPoint = { bucket_start: string; count: number };
export type AnalyticsMessageBucketResponse = {
  guild_id: string;
  from: string;
  to: string;
  bucket_size: number;
  channel_id?: string | null;
  points: AnalyticsMessageBucketPoint[];
};

export type ModerationLogItem = {
  id: number;
  message_id: string | null;
  channel_id: string | null;
  author_id: string | null;
  actor_id?: string | null;
  actor_type?: string | null;
  target_id?: string | null;
  bot_id?: string | null;
  source?: string | null;
  metadata?: Record<string, unknown> | null;
  action: string | null;
  reason: string | null;
  created_at: string;
};

export type ModerationLogUser = {
  id: string;
  username?: string | null;
  global_name?: string | null;
  discriminator?: string | null;
  avatar?: string | null;
  bot?: boolean | null;
};

export type ModerationLogsResponse = {
  guild_id: string;
  items: ModerationLogItem[];
  users?: Record<string, ModerationLogUser>;
};

export type AppealItem = {
  id: string;
  user_id: string;
  user_name?: string | null;
  user_avatar?: string | null;
  moderator_id?: string | null;
  moderator_name?: string | null;
  ban_reason?: string | null;
  appeal_text: string;
  status: string;
  summary?: string | null;
  resolved_by?: string | null;
  resolved_at?: string | null;
  created_at: string;
  updated_at: string;
};

export type AppealsResponse = { guild_id: string; items: AppealItem[] };

export type GuildSettings = {
  guild_id: string;
  prefix: string;
  language: string;
  timezone: string;
  analytics_enabled: boolean;
  sentiment_enabled: boolean;
  moderation_enabled: boolean;
  warn_decay_days?: number;
  warn_policy?: {
    threshold: number;
    action: "timeout" | "ban";
    duration_hours?: number;
  }[];
  welcome_channel_id?: string;
  log_channel_id?: string;
};
