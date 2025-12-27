import Link from "next/link";
import { Button } from "@/components/ui/button";
import { Check, X, HelpCircle, Zap, ChevronDown } from "lucide-react";
import { LOGIN_URL } from "@/lib/api";
import { backendFetch } from "@/lib/backend.server";
import { MeResponse } from "@/lib/types";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { AutoStartPlanCheckout } from "@/components/auto-start-plan-checkout";
import { CheckoutPlanButton } from "@/components/checkout-plan-button";

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

export default async function PricingPage(props: {
  searchParams?: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const searchParams = await props.searchParams;
  const checkout = typeof searchParams?.checkout === "string" ? searchParams.checkout : undefined;
  const checkoutPlan = (checkout === "plus" || checkout === "premium") ? checkout : undefined;
  const me = await getMe();
  const displayName = me?.username || me?.user_id || "Account";
  const avatarUrl =
    me?.avatar && me?.user_id
      ? `https://cdn.discordapp.com/avatars/${me.user_id}/${me.avatar}.png?size=96`
      : null;
  const dashboardHref = me ? "/dashboard" : LOGIN_URL;

  return (
    <div className="flex min-h-screen flex-col">
      <AutoStartPlanCheckout enabled={!!checkoutPlan} plan={checkoutPlan} />
      <header className="px-6 h-16 flex items-center border-b fixed w-full bg-background/80 backdrop-blur-sm z-50">
        <div className="max-w-7xl w-full mx-auto flex items-center justify-between">
          <Link href="/" className="flex items-center gap-2 font-bold text-xl">
            <img src="/logo.svg" alt="Guildest Logo" className="h-6 w-6" />
            <span>Guildest</span>
          </Link>
          <nav className="hidden md:flex items-center gap-6 text-sm font-medium">
            <Link href="/#features" className="text-muted-foreground hover:text-foreground transition-colors">Features</Link>
            <Link href="/pricing" className="text-foreground transition-colors">Pricing</Link>
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

      <main className="flex-1 pt-32 pb-20 px-6">
        <div className="max-w-7xl mx-auto space-y-16">
          <div className="text-center space-y-4">
            <h1 className="text-4xl md:text-5xl font-bold tracking-tight">Choose the right plan for your community</h1>
            <p className="text-muted-foreground max-w-2xl mx-auto text-lg">
              Unlock the full potential of your server with advanced analytics and AI-powered insights.
            </p>
            <div className="inline-block bg-yellow-500/10 text-yellow-500 border border-yellow-500/20 px-4 py-2 rounded-md text-sm font-medium mt-4">
              ⚠️ Note: These prices are not final. Users should expect different pricing versions upcoming.
            </div>
          </div>

          {/* Pricing Cards */}
          <div className="grid md:grid-cols-3 gap-8 max-w-6xl mx-auto">
            {/* Free Plan */}
            <Card className="flex flex-col">
              <CardHeader>
                <CardTitle className="text-2xl">Free</CardTitle>
                <CardDescription>Just enough to see value</CardDescription>
              </CardHeader>
              <CardContent className="flex-1 space-y-4">
                <div className="text-3xl font-bold">$0 <span className="text-base font-normal text-muted-foreground">/mo</span></div>
                <Link href={dashboardHref}>
                  <Button className="w-full" variant="outline">Get Started</Button>
                </Link>
              </CardContent>
            </Card>

            {/* Plus Plan */}
            <Card className="flex flex-col border-secondary relative overflow-hidden bg-secondary/5">
              <div className="absolute top-0 right-0 bg-secondary text-secondary-foreground px-3 py-1 text-xs font-medium rounded-bl-lg">
                POPULAR
              </div>
              <CardHeader>
                <CardTitle className="text-2xl">Plus</CardTitle>
                <CardDescription>Unlocks AI & detailed stats</CardDescription>
              </CardHeader>
              <CardContent className="flex-1 space-y-4">
                <div className="text-3xl font-bold">$9 <span className="text-base font-normal text-muted-foreground">/mo</span></div>
                <CheckoutPlanButton plan="plus" variant="secondary" label="Upgrade to Plus" redirectAfterLogin="/pricing?checkout=plus" />
              </CardContent>
            </Card>

            {/* Premium Plan */}
            <Card className="flex flex-col border-primary bg-primary/5">
              <CardHeader>
                <CardTitle className="text-2xl">Premium</CardTitle>
                <CardDescription>For power users & scale</CardDescription>
              </CardHeader>
              <CardContent className="flex-1 space-y-4">
                <div className="text-3xl font-bold">$25 <span className="text-base font-normal text-muted-foreground">/mo</span></div>
                <CheckoutPlanButton plan="premium" label="Upgrade to Premium" redirectAfterLogin="/pricing?checkout=premium" />
              </CardContent>
            </Card>
          </div>

          {/* Feature Comparison Table */}
          <div className="max-w-4xl mx-auto">
            <h2 className="text-2xl font-bold mb-8 text-center">Feature Comparison</h2>
            <div className="rounded-xl border bg-card overflow-hidden">
              <div className="overflow-x-auto">
                <table className="w-full text-sm text-left">
                  <thead className="bg-muted/50 text-muted-foreground">
                    <tr>
                      <th className="px-6 py-4 font-medium">Feature</th>
                      <th className="px-6 py-4 font-medium text-center">Free</th>
                      <th className="px-6 py-4 font-medium text-center text-secondary">Plus</th>
                      <th className="px-6 py-4 font-medium text-center text-primary">Premium</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y">
                    <tr className="hover:bg-muted/20">
                      <td className="px-6 py-4 font-medium">Servers</td>
                      <td className="px-6 py-4 text-center">1</td>
                      <td className="px-6 py-4 text-center font-medium">3</td>
                      <td className="px-6 py-4 text-center font-bold">10</td>
                    </tr>
                    <tr className="hover:bg-muted/20">
                      <td className="px-6 py-4 font-medium">Analytics History</td>
                      <td className="px-6 py-4 text-center">7 days</td>
                      <td className="px-6 py-4 text-center font-medium">30 days</td>
                      <td className="px-6 py-4 text-center font-bold">90 days</td>
                    </tr>
                    <tr className="hover:bg-muted/20">
                      <td className="px-6 py-4 font-medium">AI Reports</td>
                      <td className="px-6 py-4 text-center text-muted-foreground"><X className="h-5 w-5 mx-auto" /></td>
                      <td className="px-6 py-4 text-center font-medium">50 / mo</td>
                      <td className="px-6 py-4 text-center font-bold">500 / mo</td>
                    </tr>
                    <tr className="hover:bg-muted/20">
                      <td className="px-6 py-4 font-medium">Sentiment Tracking</td>
                      <td className="px-6 py-4 text-center text-muted-foreground"><X className="h-5 w-5 mx-auto" /></td>
                      <td className="px-6 py-4 text-center text-green-500"><Check className="h-5 w-5 mx-auto" /></td>
                      <td className="px-6 py-4 text-center text-green-500"><Check className="h-5 w-5 mx-auto" /></td>
                    </tr>
                    <tr className="hover:bg-muted/20">
                      <td className="px-6 py-4 font-medium">Moderation Logs</td>
                      <td className="px-6 py-4 text-center text-muted-foreground"><X className="h-5 w-5 mx-auto" /></td>
                      <td className="px-6 py-4 text-center font-medium">200 / mo</td>
                      <td className="px-6 py-4 text-center font-bold">500 / mo</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </div>

          {/* FAQ Section */}
          <div className="max-w-3xl mx-auto space-y-8">
            <div className="text-center">
              <h2 className="text-2xl font-bold">Frequently Asked Questions</h2>
            </div>
            <div className="grid gap-6">
              <Card>
                <CardHeader>
                  <CardTitle className="text-base flex items-center gap-2">
                    <HelpCircle className="h-4 w-4 text-muted-foreground" />
                    How are AI reports counted?
                  </CardTitle>
                </CardHeader>
                <CardContent className="text-muted-foreground text-sm">
                  Each time you generate a sentiment analysis summary or a community health report, it counts as one AI report. Quotas reset at the start of each billing cycle.
                </CardContent>
              </Card>
              <Card>
                <CardHeader>
                  <CardTitle className="text-base flex items-center gap-2">
                    <HelpCircle className="h-4 w-4 text-muted-foreground" />
                    What happens if I exceed my limits?
                  </CardTitle>
                </CardHeader>
                <CardContent className="text-muted-foreground text-sm">
                  We'll notify you when you're close to your limit. AI features will be paused until the next cycle or if you upgrade your plan. Data collection continues uninterrupted.
                </CardContent>
              </Card>
              <Card>
                <CardHeader>
                  <CardTitle className="text-base flex items-center gap-2">
                    <HelpCircle className="h-4 w-4 text-muted-foreground" />
                    Can I cancel anytime?
                  </CardTitle>
                </CardHeader>
                <CardContent className="text-muted-foreground text-sm">
                  Yes, you can cancel your subscription at any time. You'll retain access to your paid features until the end of your current billing period.
                </CardContent>
              </Card>
            </div>
          </div>
        </div>
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
