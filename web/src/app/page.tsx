import { cookies } from "next/headers";
import Image from "next/image";
import { getDashboardMe, getPublicLinks, getPublicMessageHeatmap } from "@/lib/public-api";

function Heatmap({ days }: { days: Array<{ date: string; message_count: number }> }) {
  const max = Math.max(...days.map((d) => d.message_count), 1);

  function cellColor(count: number) {
    if (count === 0) return "bg-surface-light";
    const level = Math.ceil((count / max) * 4);
    if (level >= 4) return "bg-tan";
    if (level === 3) return "bg-tan/70";
    if (level === 2) return "bg-tan/40";
    return "bg-tan/20";
  }

  return (
    <div className="flex gap-[2px] w-full">
      {days.map((d) => (
        <div
          key={d.date}
          className={`flex-1 h-8 rounded-[2px] min-w-0 ${cellColor(d.message_count)}`}
          title={`${d.date}: ${d.message_count} messages`}
        />
      ))}
    </div>
  );
}

export default async function Home() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const [links, dashboard, heatmap] = await Promise.all([
    getPublicLinks(),
    getDashboardMe(cookieHeader),
    getPublicMessageHeatmap(365),
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

      {/* Heatmap */}
      <section className="px-8 pb-20">
        <Heatmap days={heatmap.days} />
        <div className="mt-2 flex justify-between text-[10px] text-cream/25">
          <span>{heatmap.days[0]?.date ?? ""}</span>
          <span>{heatmap.total_messages.toLocaleString()} messages tracked</span>
          <span>{heatmap.days[heatmap.days.length - 1]?.date ?? ""}</span>
        </div>
      </section>
    </div>
  );
}
