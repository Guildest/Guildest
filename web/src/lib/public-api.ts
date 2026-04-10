export type PublicLinks = {
  install_url: string;
  invite_url: string;
  login_url: string;
};

export type AccessibleGuild = {
  guild_id: string;
  guild_name: string;
  is_owner: boolean;
  member_count: number;
};

export type DashboardUser = {
  display_name: string;
  discord_user_id: string;
  username: string;
};

export type DashboardMe = {
  accessible_guilds: AccessibleGuild[];
  user: DashboardUser;
};

export type GuildMessageSummary = {
  backfill_status: string | null;
  daily: Array<{
    date: string;
    message_count: number;
  }>;
  days_requested: number;
  guild_id: string;
  total_messages: number;
};

export type PublicStats = {
  members: number;
  messages_tracked: number;
  servers: number;
};

export type GuildUsersSummary = {
  avg_messages_per_active_user: number;
  days_requested: number;
  guild_id: string;
  total_active_users: number;
  total_voice_seconds: number;
  users: Array<{
    active_days: number;
    discord_user_id: string;
    label: string;
    secondary_label: string | null;
    messages_sent: number;
    reactions_added: number;
    voice_seconds: number;
  }>;
};

export type GuildActivationFunnel = {
  days_requested: number;
  guild_id: string;
  steps: Array<{
    count: number;
    key: string;
    label: string;
  }>;
};

export type GuildHealthSummary = {
  dau: number;
  days_requested: number;
  guild_id: string;
  join_leave_ratio: number | null;
  joined_members: number;
  left_members: number;
  onboarding_completion_rate: number | null;
  onboarded_members: number;
  wau: number;
  mau: number;
};

export type AiLivePulse = {
  window_start: string;
  window_end: string;
  window_minutes: number;
  total_observations: number;
  classified_count: number;
  question_count: number;
  feedback_count: number;
  support_count: number;
  positive_sentiment_count: number;
  negative_sentiment_count: number;
  neutral_sentiment_count: number;
  high_urgency_count: number;
};

export type GuildRetentionCohorts = {
  cohorts: Array<{
    cohort_age_days: number;
    cohort_date: string;
    d30_retained: number;
    d30_retention_rate: number | null;
    d7_retained: number;
    d7_retention_rate: number | null;
    joined_count: number;
  }>;
  d30_retention_rate: number | null;
  d7_retention_rate: number | null;
  days_requested: number;
  guild_id: string;
};

export type GuildHotspots = {
  active_channels: number;
  days_requested: number;
  guild_id: string;
  hourly_activity: Array<{
    hour_label: string;
    hour_of_day: number;
    message_count: number;
  }>;
  peak_hour_utc: string | null;
  retention_channels: Array<{
    channel_id: string;
    d30_retained_members: number;
    d7_retained_members: number;
    label: string;
    retention_score: number;
  }>;
  top_channels: Array<{
    avg_response_seconds: number | null;
    channel_id: string;
    health_score: number;
    label: string;
    message_count: number;
    messages_per_sender: number | null;
    previous_period_messages: number;
    replies: number;
    trend_percent_change: number | null;
    unique_senders: number;
  }>;
  total_messages: number;
};

export type GuildPipelineHealth = {
  guild_id: string;
  healthy_streams: number;
  max_oldest_ready_age_seconds: number;
  max_scheduled_retry_overdue_seconds: number;
  overall_status: string;
  streams: Array<{
    dead_letter_messages: number;
    label: string;
    oldest_dead_letter_age_seconds: number;
    oldest_ready_age_seconds: number;
    pending_messages: number;
    ready_messages: number;
    scheduled_retry_messages: number;
    scheduled_retry_overdue_seconds: number;
    status: string;
    stream: string;
  }>;
  total_dead_letter_messages: number;
  total_pending_messages: number;
  total_ready_messages: number;
  total_scheduled_retry_messages: number;
  total_streams: number;
};

export type GuildPipelineIncidents = {
  guild_id: string;
  incidents: Array<{
    age_seconds: number;
    attempts: number;
    dead_letter_entry_id: string;
    delivery_id: string;
    error: string;
    failed_at: string;
    payload_preview: string;
    retry_key: string;
    source_stream: string;
    source_stream_label: string;
  }>;
  total_dead_letter_messages: number;
};

export type GuildPipelineReplayHistory = {
  guild_id: string;
  replays: Array<{
    attempts: number;
    delivery_id: string;
    operator_reason: string | null;
    replayed_at: string;
    replayed_by_label: string;
    replayed_by_user_id: string | null;
    source_stream: string;
    source_stream_label: string;
  }>;
};

export type GuildPipelineDiscardHistory = {
  discards: Array<{
    attempts: number;
    delivery_id: string;
    discarded_at: string;
    discarded_by_label: string;
    discarded_by_user_id: string | null;
    operator_reason: string | null;
    source_stream: string;
    source_stream_label: string;
  }>;
  guild_id: string;
};

function getApiBaseUrl() {
  return process.env.GUILDEST_API_BASE_URL ?? "http://127.0.0.1:8080";
}

export async function getPublicLinks(): Promise<PublicLinks> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/v1/public/links`, {
      next: { revalidate: 60 },
    });

    if (!response.ok) {
      throw new Error(`links request failed: ${response.status}`);
    }

    return (await response.json()) as PublicLinks;
  } catch {
    return {
      install_url: "#",
      invite_url: "#",
      login_url: "#",
    };
  }
}

export async function getPublicStats(): Promise<PublicStats> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/v1/public/stats`, {
      next: { revalidate: 60 },
    });

    if (!response.ok) {
      throw new Error(`stats request failed: ${response.status}`);
    }

    return (await response.json()) as PublicStats;
  } catch {
    return {
      members: 0,
      messages_tracked: 0,
      servers: 0,
    };
  }
}

export async function getDashboardMe(
  cookieHeader?: string,
): Promise<DashboardMe | null> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/v1/dashboard/me`, {
      cache: "no-store",
      headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
    });

    if (response.status === 401) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`dashboard me request failed: ${response.status}`);
    }

    return (await response.json()) as DashboardMe;
  } catch {
    return null;
  }
}

export async function getGuildMessageSummary(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildMessageSummary | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/messages/summary?days=7`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`message summary request failed: ${response.status}`);
    }

    return (await response.json()) as GuildMessageSummary;
  } catch {
    return null;
  }
}

export async function getGuildUsersSummary(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildUsersSummary | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/users/summary?days=7`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`users summary request failed: ${response.status}`);
    }

    return (await response.json()) as GuildUsersSummary;
  } catch {
    return null;
  }
}

export async function getGuildActivationFunnel(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildActivationFunnel | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/activation/funnel?days=30`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`activation funnel request failed: ${response.status}`);
    }

    return (await response.json()) as GuildActivationFunnel;
  } catch {
    return null;
  }
}

export async function getGuildHealthSummary(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildHealthSummary | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/summary/health?days=30`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`guild health request failed: ${response.status}`);
    }

    return (await response.json()) as GuildHealthSummary;
  } catch {
    return null;
  }
}

export async function getGuildRetentionCohorts(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildRetentionCohorts | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/retention/cohorts?days=30`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`retention cohorts request failed: ${response.status}`);
    }

    return (await response.json()) as GuildRetentionCohorts;
  } catch {
    return null;
  }
}

export async function getGuildHotspots(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildHotspots | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/channels/hotspots?days=7`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`hotspots request failed: ${response.status}`);
    }

    return (await response.json()) as GuildHotspots;
  } catch {
    return null;
  }
}

export async function getGuildPipelineHealth(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildPipelineHealth | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/ops/pipeline`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`pipeline health request failed: ${response.status}`);
    }

    return (await response.json()) as GuildPipelineHealth;
  } catch {
    return null;
  }
}

export async function getGuildPipelineIncidents(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildPipelineIncidents | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/ops/incidents`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`pipeline incidents request failed: ${response.status}`);
    }

    return (await response.json()) as GuildPipelineIncidents;
  } catch {
    return null;
  }
}

export async function getGuildPipelineReplayHistory(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildPipelineReplayHistory | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/ops/replays`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`pipeline replay history request failed: ${response.status}`);
    }

    return (await response.json()) as GuildPipelineReplayHistory;
  } catch {
    return null;
  }
}

export async function getGuildPipelineDiscardHistory(
  guildId: string,
  cookieHeader?: string,
): Promise<GuildPipelineDiscardHistory | null> {
  try {
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/ops/discards`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );

    if (response.status === 401 || response.status === 403) {
      return null;
    }

    if (!response.ok) {
      throw new Error(`pipeline discard history request failed: ${response.status}`);
    }

    return (await response.json()) as GuildPipelineDiscardHistory;
  } catch {
    return null;
  }
}

export async function getAiLivePulse(
  guildId: string,
  windowMinutes?: number,
  cookieHeader?: string,
): Promise<AiLivePulse | null> {
  try {
    const params = windowMinutes ? `?window_minutes=${windowMinutes}` : "";
    const response = await fetch(
      `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/ai/live-pulse${params}`,
      {
        cache: "no-store",
        headers: cookieHeader ? { Cookie: cookieHeader } : undefined,
      },
    );
    if (response.status === 401 || response.status === 403) return null;
    if (!response.ok) throw new Error(`ai live pulse request failed: ${response.status}`);
    return (await response.json()) as AiLivePulse;
  } catch {
    return null;
  }
}
