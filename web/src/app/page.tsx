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
            href={links.invite_url}
            className="flex items-center justify-center gap-3 bg-tan text-plum font-medium hover:bg-sand transition-colors rounded-2xl"
            style={{ width: 180, height: 56 }}
          >
            <span>Invite</span>
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

      {/* Heatmap */}
      <section className="pb-20">
        <Heatmap days={heatmap.days} />
        <p className="mt-2 text-center text-[10px] text-cream/25">
          {messagesTracked.toLocaleString()} messages indexed
        </p>
      </section>
    </div>
  );
}
