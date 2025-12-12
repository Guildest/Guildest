import { backendFetch } from "@/lib/backend.server";
import { MessageCountsResponse, SentimentDailyResponse } from "@/lib/types";
import { MessageVolumeChart, SentimentChart } from "@/components/analytics-charts";

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

export default async function AnalyticsPage({
  params,
}: {
  params: Promise<{ guildId: string }>;
}) {
  const { guildId } = await params;
  const [messages, sentiment] = await Promise.all([
    getMessageCounts(guildId),
    getSentimentDaily(guildId),
  ]);

  return (
    <div className="space-y-8 max-w-6xl mx-auto">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Analytics</h1>
        <p className="text-muted-foreground">
          Insights into your community activity and health.
        </p>
      </div>

      <div className="grid gap-8">
        {messages && messages.points.length > 0 ? (
          <MessageVolumeChart data={messages.points} />
        ) : (
          <div className="p-8 border rounded-lg text-center text-muted-foreground">
            No message data available yet.
          </div>
        )}

        {sentiment && sentiment.points.length > 0 ? (
          <SentimentChart data={sentiment.points} />
        ) : (
          <div className="p-8 border rounded-lg text-center text-muted-foreground">
            No sentiment data available yet.
          </div>
        )}
      </div>
    </div>
  );
}
