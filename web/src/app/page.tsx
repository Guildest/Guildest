import { cookies } from "next/headers";
import Image from "next/image";
import {
  getDashboardMe,
  getPublicLinks,
  getPublicMessageHeatmap,
  getPublicStats,
} from "@/lib/public-api";
import { Heatmap } from "@/components/heatmap";

export default async function Home() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const [links, dashboard, heatmap, stats] = await Promise.all([
    getPublicLinks(),
    getDashboardMe(cookieHeader),
    getPublicMessageHeatmap(365),
    getPublicStats(),
  ]);

  const loginHref = dashboard ? "/dashboard" : links.login_url;
  const messagesTracked = stats.messages_tracked || heatmap.total_messages;

  return (
    <div className="min-h-screen bg-plum">
      {/* Logo */}
      <div className="px-8 pt-8">
        <Image src="/logolanding.svg" alt="Guildest logo" width={48} height={44} />
      </div>

      {/* Hero */}
      <section className="px-8 pt-14 pb-16">
        <h1 className="text-5xl md:text-6xl font-display leading-tight text-cream tracking-tight">
          Know what your<br />
          Discord needs next.
        </h1>
        <p className="mt-4 text-cream/50 text-lg max-w-lg leading-relaxed">
          Guildest maps your server, indexes the conversations that matter, and
          turns community activity into live pulses, alerts, and clear next steps.
        </p>

        <div className="flex gap-3 mt-8">
          <a
            href="/waitlist"
            className="flex items-center justify-center gap-3 bg-tan text-plum font-medium hover:bg-sand transition-colors rounded-2xl"
            style={{ width: 200, height: 56 }}
          >
            <span>Join waitlist</span>
            <Image src="/arrow.svg" alt="" width={24} height={24} />
          </a>
          <a
            href={loginHref}
            className="flex items-center justify-center gap-3 bg-surface-light border border-border-light text-cream font-medium hover:bg-surface transition-colors rounded-2xl"
            style={{ width: 180, height: 56 }}
          >
            <span>Login</span>
            <Image src="/discord.svg" alt="" width={28} height={28} />
          </a>
        </div>
      </section>

      {/* Community activity - the pulse */}
      <section className="mt-12 pb-20 px-8">
        {/* Stats floating above */}
        <div className="flex items-end justify-between mb-6">
          <div className="flex items-center gap-3">
            <div className="w-1.5 h-1.5 rounded-full bg-tan/60 animate-pulse" />
            <span className="text-[11px] text-cream/40 tracking-widest uppercase">Live pulse</span>
          </div>
          <div className="flex items-baseline gap-8">
            <div className="text-right">
              <span className="text-3xl font-display text-tan">{messagesTracked.toLocaleString()}</span>
              <p className="text-[10px] text-cream/25 mt-0.5">messages</p>
            </div>
          </div>
        </div>

        {/* Heatmap - the only density in vast space */}
        <div className="border border-border-light/30 rounded-xl p-6 bg-surface-light/[0.02]">
          <Heatmap days={heatmap.days} servers={stats.servers} members={stats.members} />
        </div>

        {/* Subtle footer line */}
        <div className="flex items-center justify-between mt-4 text-[10px] text-cream/20">
          <span>{stats.servers.toLocaleString()} communities · {stats.members.toLocaleString()} members</span>
          <span>Last 365 days</span>
        </div>
      </section>
    </div>
  );
}
