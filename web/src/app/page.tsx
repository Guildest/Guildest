import { cookies } from "next/headers";
import Image from "next/image";
import { getDashboardMe, getPublicLinks, getPublicStats } from "@/lib/public-api";

const numberFormatter = new Intl.NumberFormat("en-US");

export default async function Home() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const [stats, links, dashboard] = await Promise.all([
    getPublicStats(),
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

      {/* Stats Cards */}
      <section className="px-8 lg:px-16 pb-16 mt-40">
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 max-w-6xl mx-auto">
          <div className="metric-card p-8 min-h-[160px] flex flex-col justify-end">
            <p className="text-cream/35 text-[11px] font-semibold uppercase tracking-wider">Members</p>
            <p className="text-cream text-3xl font-semibold tracking-tight mt-2">
              {numberFormatter.format(stats.members)}
            </p>
          </div>
          <div className="metric-card p-8 min-h-[160px] flex flex-col justify-end">
            <p className="text-cream/35 text-[11px] font-semibold uppercase tracking-wider">Servers</p>
            <p className="text-cream text-3xl font-semibold tracking-tight mt-2">
              {numberFormatter.format(stats.servers)}
            </p>
          </div>
          <div className="metric-card p-8 min-h-[160px] flex flex-col justify-end">
            <p className="text-cream/35 text-[11px] font-semibold uppercase tracking-wider">Messages</p>
            <p className="text-cream text-3xl font-semibold tracking-tight mt-2">
              {numberFormatter.format(stats.messages_tracked)}
            </p>
          </div>
        </div>
      </section>
    </div>
  );
}
