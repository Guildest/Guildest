import { cookies } from "next/headers";
import Image from "next/image";
import Link from "next/link";
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
      {/* Top nav */}
      <nav className="px-8 pt-6 pb-2 flex items-center justify-between max-w-7xl mx-auto">
        <Link href="/" className="flex items-center gap-2">
          <Image src="/logolanding.svg" alt="Guildest" width={36} height={32} />
          <span className="text-cream font-display text-lg tracking-tight">Guildest</span>
        </Link>
        <div className="hidden md:flex items-center gap-8 text-sm text-cream/60">
          <a href="#features" className="hover:text-cream transition-colors">Features</a>
          <a href="#why" className="hover:text-cream transition-colors">Why Guildest</a>
          <a href="#pulse" className="hover:text-cream transition-colors">Live pulse</a>
          <a href={loginHref} className="hover:text-cream transition-colors">Login</a>
        </div>
        <Link
          href="/waitlist"
          className="bg-tan text-plum text-sm font-medium px-4 py-2 rounded-full hover:bg-sand transition-colors"
        >
          Join waitlist
        </Link>
      </nav>

      {/* Hero */}
      <section className="px-8 pt-20 pb-24 max-w-5xl mx-auto text-center">
        <div className="inline-flex items-center gap-2 border border-border-light/40 rounded-full px-3 py-1 mb-8 text-[11px] text-cream/50 tracking-widest uppercase">
          <span className="w-1.5 h-1.5 rounded-full bg-tan animate-pulse" />
          Early access — closed beta
        </div>
        <h1 className="text-5xl md:text-7xl font-display leading-[1.05] text-cream tracking-tight">
          Building better<br />Discord communities, with AI.
        </h1>
        <p className="mt-6 text-cream/55 text-lg max-w-2xl mx-auto leading-relaxed">
          Guildest reads your server, surfaces what matters, and turns activity
          into live pulses, alerts, and clear next steps — so you spend less time
          watching chat and more time building.
        </p>
        <div className="flex flex-wrap items-center justify-center gap-3 mt-10">
          <Link
            href="/waitlist"
            className="flex items-center justify-center gap-3 bg-tan text-plum font-medium hover:bg-sand transition-colors rounded-2xl"
            style={{ width: 200, height: 56 }}
          >
            <span>Join waitlist</span>
            <Image src="/arrow.svg" alt="" width={24} height={24} />
          </Link>
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

      {/* Proof strip */}
      <section className="px-8 pb-20 max-w-5xl mx-auto">
        <p className="text-center text-[11px] text-cream/30 tracking-widest uppercase mb-6">
          Built for community-led teams
        </p>
        <div className="flex flex-wrap justify-center items-center gap-x-12 gap-y-4 text-cream/35 text-sm">
          <span>Creators</span>
          <span>·</span>
          <span>Indie SaaS</span>
          <span>·</span>
          <span>YC startups</span>
          <span>·</span>
          <span>Web3 communities</span>
          <span>·</span>
          <span>Gaming guilds</span>
        </div>
      </section>

      {/* Feature visual — heatmap as the centerpiece */}
      <section id="pulse" className="px-8 pb-24 max-w-6xl mx-auto">
        <div className="text-center mb-10">
          <h2 className="text-3xl md:text-4xl font-display text-cream tracking-tight">
            The pulse of every community,<br />in one view.
          </h2>
          <p className="mt-4 text-cream/50 max-w-xl mx-auto">
            One year of activity across every Guildest server. This is what your
            community would look like — visible, measurable, alive.
          </p>
        </div>

        <div className="flex items-end justify-between mb-6">
          <div className="flex items-center gap-3">
            <div className="w-1.5 h-1.5 rounded-full bg-tan/60 animate-pulse" />
            <span className="text-[11px] text-cream/40 tracking-widest uppercase">Live pulse</span>
          </div>
          <div className="text-right">
            <span className="text-3xl font-display text-tan">{messagesTracked.toLocaleString()}</span>
            <p className="text-[10px] text-cream/25 mt-0.5">messages</p>
          </div>
        </div>

        <div className="border border-border-light/30 rounded-xl p-6 bg-surface-light/[0.02]">
          <Heatmap days={heatmap.days} servers={stats.servers} members={stats.members} />
        </div>

        <div className="flex items-center justify-between mt-4 text-[10px] text-cream/20">
          <span>{stats.servers.toLocaleString()} communities · {stats.members.toLocaleString()} members</span>
          <span>Last 365 days</span>
        </div>
      </section>

      {/* Why Guildest — feature grid */}
      <section id="why" className="px-8 pb-24 max-w-6xl mx-auto">
        <div className="text-center mb-14">
          <h2 className="text-3xl md:text-4xl font-display text-cream tracking-tight">
            Why Guildest?
          </h2>
          <p className="mt-4 text-cream/50 max-w-xl mx-auto">
            Discord wasn&apos;t built for community managers. Guildest is.
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <FeatureCard
            title="AI live pulse"
            body="Every conversation classified — questions, feedback, support — in near real time."
          />
          <FeatureCard
            title="Built for managers"
            body="Health, retention, hotspots, onboarding funnels. The numbers you actually need."
          />
          <FeatureCard
            title="Alerts that matter"
            body="Get pinged when sentiment dips or a key channel goes quiet. Skip the rest."
          />
          <FeatureCard
            title="Made for action"
            body="Not just charts — clear next steps you or your AI advisor can take today."
          />
        </div>
      </section>

      {/* Big banner */}
      <section className="px-8 pb-24 max-w-7xl mx-auto">
        <div className="relative overflow-hidden rounded-3xl border border-border-light/40 bg-gradient-to-br from-surface to-plum-light px-8 py-20 md:py-28 text-center">
          <div
            aria-hidden
            className="absolute inset-0 opacity-30"
            style={{
              background:
                "radial-gradient(ellipse 60% 50% at 50% 0%, rgba(212, 165, 116, 0.25), transparent 70%)",
            }}
          />
          <div className="relative">
            <p className="text-[11px] text-tan/70 tracking-widest uppercase mb-4">
              From your first server to your largest community
            </p>
            <h2 className="text-3xl md:text-5xl font-display text-cream tracking-tight max-w-3xl mx-auto leading-[1.1]">
              Everything happens<br />inside one dashboard.
            </h2>
          </div>
        </div>
      </section>

      {/* Final CTA */}
      <section id="features" className="px-8 pb-32 max-w-3xl mx-auto text-center">
        <h2 className="text-4xl md:text-5xl font-display text-cream tracking-tight">
          Let&apos;s create<br />better communities.
        </h2>
        <p className="mt-5 text-cream/55 max-w-lg mx-auto">
          Spots are limited during closed beta. Sign up with Discord, tell us
          what you&apos;re building, and we&apos;ll let you in.
        </p>
        <div className="mt-8">
          <Link
            href="/waitlist"
            className="inline-flex items-center justify-center gap-3 bg-tan text-plum font-medium hover:bg-sand transition-colors rounded-2xl"
            style={{ width: 220, height: 56 }}
          >
            <span>Join waitlist</span>
            <Image src="/arrow.svg" alt="" width={24} height={24} />
          </Link>
        </div>
      </section>

      <footer className="px-8 py-10 border-t border-border-light/30">
        <div className="max-w-7xl mx-auto flex items-center justify-between text-[12px] text-cream/30">
          <div className="flex items-center gap-2">
            <Image src="/logolanding.svg" alt="" width={20} height={18} />
            <span>Guildest © {new Date().getFullYear()}</span>
          </div>
          <div className="flex gap-6">
            <a href={loginHref} className="hover:text-cream/60 transition-colors">Login</a>
            <Link href="/waitlist" className="hover:text-cream/60 transition-colors">Waitlist</Link>
          </div>
        </div>
      </footer>
    </div>
  );
}

function FeatureCard({ title, body }: { title: string; body: string }) {
  return (
    <div className="border border-border-light/30 rounded-2xl p-6 bg-surface-light/[0.02] hover:bg-surface-light/[0.05] transition-colors">
      <h3 className="text-cream font-display text-lg tracking-tight">{title}</h3>
      <p className="mt-3 text-sm text-cream/50 leading-relaxed">{body}</p>
    </div>
  );
}
