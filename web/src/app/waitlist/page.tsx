import { cookies } from "next/headers";
import Image from "next/image";
import Link from "next/link";
import { getDashboardMe, getPublicLinks } from "@/lib/public-api";
import { WaitlistForm } from "./waitlist-form";

export default async function WaitlistPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const [links, dashboard] = await Promise.all([
    getPublicLinks(),
    getDashboardMe(cookieHeader),
  ]);

  return (
    <div className="min-h-screen bg-plum">
      <div className="px-8 pt-8">
        <Link href="/" className="inline-block">
          <Image src="/logolanding.svg" alt="Guildest logo" width={48} height={44} />
        </Link>
      </div>

      <section className="px-8 pt-14 pb-20 max-w-xl">
        <p className="text-[11px] text-cream/40 tracking-widest uppercase mb-4">
          Early access
        </p>
        <h1 className="text-5xl md:text-6xl font-display leading-tight text-cream tracking-tight">
          Let&apos;s create<br />better communities.
        </h1>
        <p className="mt-5 text-cream/50 text-lg leading-relaxed">
          Guildest turns your Discord into a living organism — AI that reads the
          room, surfaces what matters, and helps you act on it. Join the waitlist.
        </p>

        <div className="mt-10">
          {dashboard ? (
            <WaitlistForm
              displayName={dashboard.user.display_name || dashboard.user.username}
              discordUserId={dashboard.user.discord_user_id}
            />
          ) : (
            <SignInWithDiscord loginUrl={links.login_url} />
          )}
        </div>
      </section>
    </div>
  );
}

function SignInWithDiscord({ loginUrl }: { loginUrl: string }) {
  return (
    <div>
      <a
        href={loginUrl}
        className="inline-flex items-center justify-center gap-3 bg-tan text-plum font-medium hover:bg-sand transition-colors rounded-2xl"
        style={{ width: 280, height: 56 }}
      >
        <Image src="/discord.svg" alt="" width={24} height={24} />
        <span>Sign up with Discord</span>
      </a>
      <p className="mt-3 text-[12px] text-cream/30">
        We use Discord to verify you&apos;re a real human running a real server.
      </p>
    </div>
  );
}
