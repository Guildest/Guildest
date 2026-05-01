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
    <div className="relative min-h-screen bg-plum text-cream overflow-hidden">
      <div aria-hidden className="absolute inset-0 ascii-bg pointer-events-none opacity-60" />

      <div className="relative px-6 md:px-10 pt-8 pb-6 flex items-center justify-between">
        <Link href="/" className="flex items-center">
          <Image src="/logolanding.svg" alt="Guildest" width={32} height={30} />
        </Link>
        <Link
          href="/"
          className="text-[13px] text-cream/55 hover:text-cream transition-colors"
        >
          ← Back
        </Link>
      </div>

      <main className="relative max-w-xl mx-auto px-6 pt-12 md:pt-20 pb-24">
        <h1 className="font-display text-5xl md:text-6xl leading-[1.02] tracking-tight text-cream">
          Let&apos;s create<br />better communities.
        </h1>
        <p className="mt-6 text-cream/55 leading-relaxed">
          Guildest reads the room, surfaces what matters, and helps you act on
          it. Tell us a bit about you and we&apos;ll get you in.
        </p>

        <div className="mt-12">
          {dashboard ? (
            <WaitlistForm
              displayName={dashboard.user.display_name || dashboard.user.username}
              discordUserId={dashboard.user.discord_user_id}
            />
          ) : (
            <SignInWithDiscord loginUrl={links.login_url} />
          )}
        </div>
      </main>
    </div>
  );
}

function SignInWithDiscord({ loginUrl }: { loginUrl: string }) {
  return (
    <div>
      <a
        href={loginUrl}
        className="inline-flex items-center justify-center bg-cream text-plum text-sm font-medium px-6 py-3 hover:bg-cream/90 transition-colors"
      >
        Sign up with Discord
      </a>
      <p className="mt-4 text-[12px] text-cream/35 max-w-sm leading-relaxed">
        We use Discord to verify you&apos;re a real human running a real server.
      </p>
    </div>
  );
}
