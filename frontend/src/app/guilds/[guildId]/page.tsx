import Link from "next/link";
import { redirect } from "next/navigation";

import { SimpleLineChart } from "@/components/Charts";
import { backendFetch } from "@/lib/backend.server";
import type {
  DashboardOverview,
  MessageCountsResponse,
  ModerationLogsResponse,
  SentimentDailyResponse,
} from "@/lib/types";

async function mustJson<T>(res: Response): Promise<T> {
  if (res.status === 401) redirect("/");
  if (res.status === 403) redirect("/dashboard");
  if (!res.ok) throw new Error(`Request failed (${res.status})`);
  return res.json();
}

export default async function GuildPage({ params }: { params: Promise<{ guildId: string }> }) {
  const { guildId } = await params;

  const overview = await mustJson<DashboardOverview>(await backendFetch(`/guilds/${guildId}/dashboard/overview`));
  const messageCounts = await mustJson<MessageCountsResponse>(
    await backendFetch(`/guilds/${guildId}/analytics/message-counts?hours=168`)
  );
  const sentimentDaily = await mustJson<SentimentDailyResponse>(
    await backendFetch(`/guilds/${guildId}/sentiment/daily?days=30`)
  );

  let moderationLogs: ModerationLogsResponse | null = null;
  const moderationRes = await backendFetch(`/guilds/${guildId}/moderation/logs?limit=100`);
  if (moderationRes.ok) moderationLogs = await moderationRes.json();

  let sentimentReport: unknown | null = null;
  const reportRes = await backendFetch(`/guilds/${guildId}/sentiment/report`);
  if (reportRes.ok) sentimentReport = (await reportRes.json()).report ?? null;

  const messageData = messageCounts.points.map((p) => ({
    t: new Date(p.time_bucket).toLocaleString(),
    count: p.count,
  }));
  const sentimentData = sentimentDaily.points.map((p) => ({
    d: p.day,
    score: p.score ?? 0,
  }));

  return (
    <div className="min-h-screen bg-zinc-50 text-zinc-950">
      <main className="mx-auto flex max-w-5xl flex-col gap-8 px-6 py-10">
        <header className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-xs text-zinc-600">Guild</div>
              <h1 className="text-2xl font-semibold tracking-tight">{guildId}</h1>
            </div>
            <Link className="text-sm font-medium text-zinc-900 underline" href="/dashboard">
              Back
            </Link>
          </div>
          <p className="text-sm text-zinc-600">
            Plan: <span className="font-medium">{overview.plan}</span>
          </p>
        </header>

        <section className="rounded-2xl border bg-white p-6">
          <h2 className="text-lg font-semibold">Message volume (last 7 days)</h2>
          <div className="mt-4">
            <SimpleLineChart data={messageData} xKey="t" yKey="count" />
          </div>
        </section>

        <section className="rounded-2xl border bg-white p-6">
          <h2 className="text-lg font-semibold">Sentiment (last 30 days)</h2>
          <div className="mt-4">
            <SimpleLineChart data={sentimentData} xKey="d" yKey="score" />
          </div>

          <div className="mt-6">
            <h3 className="text-sm font-semibold">Daily AI report</h3>
            {overview.features.sentiment_reports ? (
              <pre className="mt-2 max-h-80 overflow-auto rounded-xl bg-zinc-950 p-4 text-xs text-zinc-50">
                {JSON.stringify(sentimentReport, null, 2)}
              </pre>
            ) : (
              <div className="mt-2 rounded-xl border bg-zinc-50 p-4 text-sm text-zinc-700">
                Locked — upgrade to Pro for detailed reports and event recommendations.
              </div>
            )}
          </div>
        </section>

        <section className="rounded-2xl border bg-white p-6">
          <h2 className="text-lg font-semibold">Moderation history</h2>
          {overview.features.moderation_logs ? (
            <div className="mt-4 grid gap-2">
              {moderationLogs?.items?.length ? (
                moderationLogs.items.map((item) => (
                  <div key={item.id} className="rounded-xl border p-3">
                    <div className="flex items-center justify-between gap-4">
                      <div className="text-xs font-medium">{item.action ?? "event"}</div>
                      <div className="text-xs text-zinc-600">{new Date(item.created_at).toLocaleString()}</div>
                    </div>
                    <div className="mt-1 text-xs text-zinc-700">{item.reason ?? ""}</div>
                  </div>
                ))
              ) : (
                <div className="text-sm text-zinc-600">No moderation events yet.</div>
              )}
            </div>
          ) : (
            <div className="mt-3 rounded-xl border bg-zinc-50 p-4 text-sm text-zinc-700">
              Locked — upgrade to Pro for per-server moderation audit history.
            </div>
          )}
        </section>
      </main>
    </div>
  );
}

