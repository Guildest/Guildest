import { cookies } from "next/headers";
import Image from "next/image";
import { getDashboardMe, getPublicLinks } from "@/lib/public-api";

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
        <Image
          src="/logolanding.svg"
          alt="Guildest logo"
          width={48}
          height={44}
        />
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

        {/* CTA Buttons */}
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

      {/* Live Pulse Preview */}
      <section className="px-8 lg:px-16 pb-16 mt-16 max-w-2xl">
        <div className="rounded-2xl border border-border bg-surface p-6 space-y-6">
          {/* Header */}
          <div className="flex items-start justify-between">
            <div>
              <h2 className="text-sm font-semibold text-cream">Live Pulse</h2>
              <p className="text-xs text-cream/40 mt-0.5">Last 60 minutes · 91% classified</p>
            </div>
            <span className="flex items-center gap-1.5 text-xs text-emerald-400 font-medium">
              <span className="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" />
              Live
            </span>
          </div>

          {/* Stat grid */}
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {[
              { label: "Messages", value: 142 },
              { label: "Questions", value: 18 },
              { label: "Feedback", value: 7 },
              { label: "High Urgency", value: 3, sub: "needs attention" },
            ].map(({ label, value, sub }) => (
              <div key={label} className="rounded-2xl border border-border bg-surface-light p-4">
                <p className="text-xs font-medium text-cream/50 uppercase tracking-wide">{label}</p>
                <p className="mt-1 text-2xl font-semibold text-cream">{value}</p>
                {sub && <p className="mt-0.5 text-xs text-cream/40">{sub}</p>}
              </div>
            ))}
          </div>

          {/* Sentiment bar */}
          <div className="space-y-2">
            <div className="flex justify-between text-xs text-cream/50">
              <span>Sentiment</span>
              <span>74+ / 38~ / 30−</span>
            </div>
            <div className="flex h-2 w-full overflow-hidden rounded-full gap-0.5">
              <div className="h-full rounded-full bg-emerald-500" style={{ width: "52%" }} />
              <div className="h-full rounded-full bg-tan/40" style={{ width: "27%" }} />
              <div className="h-full rounded-full bg-red-500" style={{ width: "21%" }} />
            </div>
          </div>
        </div>
        <p className="mt-3 text-xs text-cream/25">What Guildest sees after an hour in your server.</p>
      </section>
    </div>
  );
}
