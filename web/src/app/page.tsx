import { cookies } from "next/headers";
import Image from "next/image";
import { getDashboardMe, getPublicLinks } from "@/lib/public-api";

const signals = [
  {
    tag: "Needs attention",
    tagColor: "text-red-400 bg-red-500/10 border-red-500/20",
    dot: "bg-red-400",
    text: '4 members in #support haven\'t received a reply in over 2 hours. Common thread: onboarding confusion after the v2 update.',
    action: "Draft a pinned reply",
  },
  {
    tag: "Feedback cluster",
    tagColor: "text-tan bg-tan/10 border-tan/20",
    dot: "bg-tan",
    text: '"Custom roles" has come up 6 times across #feedback and #general this week — up from 0 last week.',
    action: "Log to roadmap",
  },
  {
    tag: "Churn signal",
    tagColor: "text-amber-400 bg-amber-500/10 border-amber-500/20",
    dot: "bg-amber-400",
    text: "A member averaging 90+ messages/month went silent 5 days ago after posting frustration about the pricing change.",
    action: "Send a DM",
  },
  {
    tag: "Positive",
    tagColor: "text-emerald-400 bg-emerald-500/10 border-emerald-500/20",
    dot: "bg-emerald-400",
    text: "Your latest #announcements post is getting 3× the usual engagement — 28 reactions, 12 follow-up messages.",
    action: "Double down",
  },
];

export default async function Home() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const [links, dashboard] = await Promise.all([
    getPublicLinks(),
    getDashboardMe(cookieHeader),
  ]);

  const loginHref = dashboard ? "/dashboard" : links.login_url;

  return (
    <div className="min-h-screen bg-plum">
      {/* Logo */}
      <div className="px-8 pt-8">
        <Image src="/logolanding.svg" alt="Guildest logo" width={48} height={44} />
      </div>

      {/* Hero */}
      <section className="px-8 pt-14 pb-16">
        <h1 className="text-5xl md:text-6xl font-display leading-tight text-cream tracking-tight">
          Build better Discord<br />
          communities. Instantly.
        </h1>
        <p className="mt-4 text-cream/50 text-lg max-w-lg leading-relaxed">
          Guildest provides the right stats, so you could correctly improve your
          community
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

      <div className="px-8 lg:px-16 max-w-5xl">

        {/* Live Pulse card */}
        <section className="pb-20">
          {/* Panel shell */}
          <div
            className="rounded-2xl overflow-hidden"
            style={{
              background: "rgba(26, 23, 36, 0.95)",
              border: "1px solid rgba(255,255,255,0.08)",
              boxShadow:
                "0 0 0 1px rgba(255,255,255,0.03), 0 32px 64px -16px rgba(0,0,0,0.6), 0 0 80px -20px rgba(212,165,116,0.08)",
            }}
          >
            {/* Panel header bar */}
            <div
              className="flex items-center justify-between px-5 py-3.5 border-b"
              style={{
                borderColor: "rgba(255,255,255,0.07)",
                background: "rgba(255,255,255,0.02)",
              }}
            >
              <div className="flex items-center gap-2.5">
                <span className="relative flex h-2 w-2">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-60" />
                  <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-400" />
                </span>
                <span className="text-xs font-semibold text-cream/70">
                  Live Pulse
                </span>
                <span className="text-xs text-cream/25">·</span>
                <span className="text-xs text-cream/35">your-server</span>
              </div>
              <span className="text-xs text-cream/25">last 60 min</span>
            </div>

            {/* Signal items */}
            <div className="divide-y" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
              {signals.map((signal, i) => (
                <div
                  key={i}
                  className="flex items-start gap-4 px-5 py-4 group"
                  style={{ background: "transparent" }}
                >
                  {/* Left dot */}
                  <div className="mt-1.5 shrink-0">
                    <span className={`block h-1.5 w-1.5 rounded-full ${signal.dot}`} />
                  </div>

                  {/* Content */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1.5 flex-wrap">
                      <span
                        className={`inline-flex items-center rounded-md px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide border ${signal.tagColor}`}
                      >
                        {signal.tag}
                      </span>
                    </div>
                    <p className="text-sm text-cream/75 leading-relaxed">
                      {signal.text}
                    </p>
                  </div>

                  {/* Action hint */}
                  <div className="shrink-0 hidden sm:flex items-center">
                    <span className="text-xs text-cream/20 group-hover:text-tan/60 transition-colors whitespace-nowrap">
                      {signal.action} →
                    </span>
                  </div>
                </div>
              ))}
            </div>

            {/* Panel footer */}
            <div
              className="px-5 py-3 flex items-center justify-between border-t"
              style={{
                borderColor: "rgba(255,255,255,0.05)",
                background: "rgba(255,255,255,0.01)",
              }}
            >
              <span className="text-xs text-cream/20">
                4 signals · 2 need a response
              </span>
              <span className="text-xs text-tan/40">
                guildest.site
              </span>
            </div>
          </div>

          {/* Caption below panel */}
          <p className="mt-4 text-center text-xs text-cream/25">
            What Guildest surfaces after watching your server for an hour.
          </p>
        </section>
      </div>
    </div>
  );
}

