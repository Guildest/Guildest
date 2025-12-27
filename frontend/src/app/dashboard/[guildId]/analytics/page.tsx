import { backendFetch } from "@/lib/backend.server";
import {
  AnalyticsCommandResponse,
  AnalyticsSummaryResponse,
  AnalyticsTopChannelsResponse,
  AnalyticsTopCommandsResponse,
  AnalyticsVoiceResponse,
  MessageCountsResponse,
  SentimentDailyResponse,
} from "@/lib/types";
import { AnalyticsDashboard } from "@/components/analytics-dashboard";

async function getMessageCounts(guildId: string): Promise<MessageCountsResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/analytics/message-counts`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

async function getSentimentDaily(guildId: string): Promise<SentimentDailyResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/sentiment/daily`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

async function getAnalyticsSummary(guildId: string): Promise<AnalyticsSummaryResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/analytics/summary?days=30`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

async function getVoiceActivity(guildId: string): Promise<AnalyticsVoiceResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/analytics/voice?hours=720&bucket_size=3600`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

async function getCommandActivity(guildId: string): Promise<AnalyticsCommandResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/analytics/commands?hours=720&bucket_size=3600`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

async function getTopChannels(guildId: string): Promise<AnalyticsTopChannelsResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/analytics/top-channels?hours=168&limit=8&bucket_size=3600`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

async function getTopCommands(guildId: string): Promise<AnalyticsTopCommandsResponse | null> {
  try {
    const res = await backendFetch(`/guilds/${guildId}/analytics/top-commands?hours=168&limit=8&bucket_size=3600`);
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

export default async function AnalyticsPage({
  params,
}: {
  params: Promise<{ guildId: string }>;
}) {
  const { guildId } = await params;
  const [messages, sentiment, summary, voice, commands, topChannels, topCommands] = await Promise.all([
    getMessageCounts(guildId),
    getSentimentDaily(guildId),
    getAnalyticsSummary(guildId),
    getVoiceActivity(guildId),
    getCommandActivity(guildId),
    getTopChannels(guildId),
    getTopCommands(guildId),
  ]);

  return (
    <div className="space-y-8 max-w-6xl mx-auto">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Analytics</h1>
        <p className="text-muted-foreground">
          Insights into your community activity and health.
        </p>
      </div>

      <AnalyticsDashboard
        guildId={guildId}
        messageCounts={messages?.points ?? []}
        sentimentPoints={sentiment?.points ?? []}
        summaryPoints={summary?.points ?? []}
        voicePoints={voice?.points ?? []}
        commandPoints={commands?.points ?? []}
        topChannels={topChannels?.points ?? []}
        topCommands={topCommands?.points ?? []}
      />
    </div>
  );
}
