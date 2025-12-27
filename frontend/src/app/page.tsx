import Link from "next/link";
import { Button } from "@/components/ui/button";
import {
  BarChart3,
  Shield,
  Bot,
  ArrowRight,
  CheckCircle2,
  Zap,
  ChevronDown,
} from "lucide-react";
import { LOGIN_URL } from "@/lib/api";
import { backendFetch } from "@/lib/backend.server";
import { MeResponse } from "@/lib/types";

async function getMe(): Promise<MeResponse | null> {
  try {
    const res = await backendFetch("/me");
    if (res.status === 401) return null;
    if (!res.ok) throw new Error(`Failed to load /me (${res.status})`);
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

export default async function Home() {
  const me = await getMe();
  const displayName = me?.username || me?.user_id || "Account";
  const avatarUrl =
    me?.avatar && me?.user_id
      ? `https://cdn.discordapp.com/avatars/${me.user_id}/${me.avatar}.png?size=96`
      : null;
  const dashboardHref = me ? "/dashboard" : LOGIN_URL;

  return (
    <div className="flex min-h-screen flex-col">
      <header className="px-6 h-16 flex items-center border-b fixed w-full bg-background/80 backdrop-blur-sm z-50">
        <div className="max-w-7xl w-full mx-auto flex items-center justify-between">
          <Link href="/" className="flex items-center gap-2 font-bold text-xl">
            <img src="/logo.svg" alt="Guildest Logo" className="h-6 w-6" />
            <span>Guildest</span>
          </Link>
          <nav className="hidden md:flex items-center gap-6 text-sm font-medium">
            <Link href="#features" className="text-muted-foreground hover:text-foreground transition-colors">Features</Link>
            <Link href="/pricing" className="text-muted-foreground hover:text-foreground transition-colors">Pricing</Link>
            <div className="relative group">
              <button
                type="button"
                className="flex items-center gap-1 rounded-full border border-muted-foreground/30 px-3 py-1 text-muted-foreground transition-colors hover:text-foreground hover:border-foreground/50"
                aria-haspopup="menu"
              >
                Resources
                <ChevronDown className="h-3 w-3" />
              </button>
              <div
                className="absolute left-0 top-full z-50 mt-2 w-44 rounded-lg border bg-background/95 p-2 shadow-lg opacity-0 pointer-events-none transition group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:opacity-100 group-focus-within:pointer-events-auto"
                role="menu"
              >
                <Link
                  href="/doc"
                  className="block rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-muted/60 hover:text-foreground transition-colors"
                  role="menuitem"
                >
                  Documentation
                </Link>
                <Link
                  href="/doc/changelog"
                  className="block rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-muted/60 hover:text-foreground transition-colors"
                  role="menuitem"
                >
                  Changelog
                </Link>
              </div>
            </div>
          </nav>
          <div className="flex items-center gap-4">
            {me ? (
              <Link href="/dashboard" className="flex items-center gap-3">
                {avatarUrl ? (
                  <img
                    src={avatarUrl}
                    alt={displayName}
                    className="h-9 w-9 rounded-full border border-primary/40"
                  />
                ) : (
                  <div className="flex h-9 w-9 items-center justify-center rounded-full bg-secondary text-sm font-semibold text-secondary-foreground">
                    {displayName.charAt(0).toUpperCase()}
                  </div>
                )}
                <div className="hidden sm:flex flex-col leading-tight">
                  <span className="text-sm font-semibold">{displayName}</span>
                  <span className="text-xs text-muted-foreground">Open dashboard</span>
                </div>
              </Link>
            ) : (
              <Link href={dashboardHref}>
                <Button>Dashboard</Button>
              </Link>
            )}
          </div>
        </div>
      </header>

      <main className="flex-1 pt-16">
        {/* Hero Section */}
        <section className="py-20 md:py-32 px-6">
          <div className="max-w-7xl mx-auto flex flex-col items-center text-center space-y-8">
            <div className="inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80">
              <span className="flex h-2 w-2 rounded-full bg-primary mr-2" />
              v1.0 Now Available
            </div>
            <h1 className="text-4xl md:text-6xl font-extrabold tracking-tight max-w-4xl">
              The Ultimate Discord Bot for <span className="text-primary">Community Growth</span>
            </h1>
            <p className="text-xl text-muted-foreground max-w-2xl">
              Powerful analytics, advanced moderation, and sentiment analysis to help you build safer and more engaged communities.
            </p>
            <div className="flex flex-col sm:flex-row gap-4 pt-4">
              <Link href={dashboardHref}>
                <Button size="lg" className="h-12 px-8 text-base gap-2">
                  Get Started <ArrowRight className="h-4 w-4" />
                </Button>
              </Link>
              <Button size="lg" variant="outline" className="h-12 px-8 text-base">
                Add to Discord
              </Button>
            </div>
          </div>
        </section>

        {/* Features Grid */}
        <section id="features" className="py-20 bg-muted/30 px-6">
          <div className="max-w-7xl mx-auto space-y-16">
            <div className="text-center space-y-4">
              <h2 className="text-3xl font-bold tracking-tight">Everything you need</h2>
              <p className="text-muted-foreground max-w-2xl mx-auto">
                Stop juggling multiple bots. Guildest provides a comprehensive suite of tools designed for modern Discord communities.
              </p>
            </div>

            <div className="grid md:grid-cols-3 gap-8">
              <div className="bg-card rounded-xl border p-8 space-y-4">
                <div className="h-12 w-12 rounded-lg bg-secondary/20 flex items-center justify-center">
                  <BarChart3 className="h-6 w-6 text-secondary" />
                </div>
                <h3 className="text-xl font-bold">Advanced Analytics</h3>
                <p className="text-muted-foreground">
                  Track message volume, active users, and growth trends with beautiful, interactive charts.
                </p>
              </div>

              <div className="bg-card rounded-xl border p-8 space-y-4">
                <div className="h-12 w-12 rounded-lg bg-primary/20 flex items-center justify-center">
                  <Bot className="h-6 w-6 text-primary" />
                </div>
                <h3 className="text-xl font-bold">Sentiment Analysis</h3>
                <p className="text-muted-foreground">
                  AI-powered sentiment tracking helps you understand the mood of your community in real-time.
                </p>
              </div>

              <div className="bg-card rounded-xl border p-8 space-y-4">
                <div className="h-12 w-12 rounded-lg bg-muted flex items-center justify-center">
                  <Shield className="h-6 w-6 text-foreground" />
                </div>
                <h3 className="text-xl font-bold">Automated Moderation</h3>
                <p className="text-muted-foreground">
                  Keep your server safe with customizable auto-mod rules, audit logs, and detailed reports.
                </p>
              </div>
            </div>
          </div>
        </section>

        {/* Pricing Section */}
        <section id="pricing" className="py-20 px-6">
           <div className="max-w-7xl mx-auto space-y-16">
            <div className="text-center space-y-4">
              <h2 className="text-3xl font-bold tracking-tight">Simple Pricing</h2>
              <p className="text-muted-foreground">
                Start for free, upgrade as you grow.
              </p>
            </div>

            <div className="grid md:grid-cols-2 gap-8 max-w-4xl mx-auto">
              {/* Free Plan */}
              <div className="rounded-xl border p-8 space-y-6 bg-card">
                <div className="space-y-2">
                  <h3 className="text-2xl font-bold">Free</h3>
                  <p className="text-muted-foreground">For small communities</p>
                </div>
                <div className="text-3xl font-bold">$0 <span className="text-base font-normal text-muted-foreground">/mo</span></div>
                <ul className="space-y-3 pt-4">
                  <li className="flex items-center gap-2">
                    <CheckCircle2 className="h-4 w-4 text-secondary" />
                    <span>Basic Analytics (7 days)</span>
                  </li>
                  <li className="flex items-center gap-2">
                    <CheckCircle2 className="h-4 w-4 text-secondary" />
                    <span>Standard Moderation</span>
                  </li>
                  <li className="flex items-center gap-2">
                    <CheckCircle2 className="h-4 w-4 text-secondary" />
                    <span>Up to 10 custom commands</span>
                  </li>
                </ul>
                <Button className="w-full" variant="outline">Get Started</Button>
              </div>

              {/* Plus Plan */}
              <div className="rounded-xl border border-primary p-8 space-y-6 relative overflow-hidden bg-card">
                <div className="absolute top-0 right-0 bg-primary text-primary-foreground px-3 py-1 text-xs font-medium rounded-bl-lg">
                  POPULAR
                </div>
                <div className="space-y-2">
                  <h3 className="text-2xl font-bold">Plus</h3>
                  <p className="text-muted-foreground">For growing servers</p>
                </div>
                <div className="text-3xl font-bold">$9 <span className="text-base font-normal text-muted-foreground">/mo</span></div>
                <ul className="space-y-3 pt-4">
                  <li className="flex items-center gap-2">
                    <CheckCircle2 className="h-4 w-4 text-primary" />
                    <span>Extended Analytics (30 days)</span>
                  </li>
                   <li className="flex items-center gap-2">
                    <CheckCircle2 className="h-4 w-4 text-primary" />
                    <span>Sentiment Analysis Reports</span>
                  </li>
                  <li className="flex items-center gap-2">
                    <CheckCircle2 className="h-4 w-4 text-primary" />
                    <span>Advanced Audit Logs</span>
                  </li>
                  <li className="flex items-center gap-2">
                    <CheckCircle2 className="h-4 w-4 text-primary" />
                    <span>Unlimited commands</span>
                  </li>
                </ul>
                <Button className="w-full">Upgrade to Plus</Button>
              </div>
            </div>
          </div>
        </section>

         {/* CTA Section */}
        <section className="py-20 px-6 bg-primary text-primary-foreground">
          <div className="max-w-4xl mx-auto text-center space-y-8">
            <h2 className="text-3xl md:text-4xl font-bold">Ready to upgrade your server?</h2>
            <p className="text-primary-foreground/80 max-w-2xl mx-auto text-lg">
              Join thousands of other community managers using Guildest to build better Discord servers.
            </p>
             <Link href={dashboardHref}>
              <Button size="lg" variant="secondary" className="h-14 px-8 text-lg font-semibold">
                Start Dashboard <Zap className="ml-2 h-4 w-4" />
              </Button>
            </Link>
          </div>
        </section>
      </main>

      <footer className="py-10 px-6 border-t bg-muted/20">
        <div className="max-w-7xl mx-auto flex flex-col md:flex-row justify-between items-center gap-6">
          <div className="flex items-center gap-2 font-semibold">
             <img src="/logo.svg" alt="Guildest Logo" className="h-5 w-5" />
            <span>Guildest</span>
          </div>
          <p className="text-sm text-muted-foreground">
            © {new Date().getFullYear()} Guildest. All rights reserved.
          </p>
        </div>
      </footer>
    </div>
  );
}
