import Link from "next/link";
import { Button } from "@/components/ui/button";
import { CheckCircle2, Zap } from "lucide-react";
import { LOGIN_URL } from "@/lib/api";

export default function PricingPage() {
  return (
    <div className="flex min-h-screen flex-col">
      <header className="px-6 h-16 flex items-center border-b fixed w-full bg-background/80 backdrop-blur-sm z-50">
        <div className="max-w-7xl w-full mx-auto flex items-center justify-between">
          <Link href="/" className="flex items-center gap-2 font-bold text-xl">
            <img src="/logo.svg" alt="Guildest Logo" className="h-6 w-6" />
            <span>Guildest</span>
          </Link>
          <nav className="hidden md:flex items-center gap-6 text-sm font-medium">
            <Link href="/#features" className="text-muted-foreground hover:text-foreground transition-colors">Features</Link>
            <Link href="/pricing" className="text-foreground transition-colors">Pricing</Link>
            <Link href="#" className="text-muted-foreground hover:text-foreground transition-colors">Documentation</Link>
          </nav>
          <div className="flex items-center gap-4">
             <Link href={LOGIN_URL}>
              <Button>Dashboard</Button>
            </Link>
          </div>
        </div>
      </header>

      <main className="flex-1 pt-32 pb-20 px-6">
        <div className="max-w-7xl mx-auto space-y-16">
          <div className="text-center space-y-4">
            <h1 className="text-4xl font-bold tracking-tight">Simple, Transparent Pricing</h1>
            <p className="text-muted-foreground max-w-2xl mx-auto text-lg">
              Choose the perfect plan for your community.
            </p>
            <div className="inline-block bg-yellow-500/10 text-yellow-500 border border-yellow-500/20 px-4 py-2 rounded-md text-sm font-medium mt-4">
              ⚠️ Note: These prices are not final. Users should expect different pricing versions upcoming.
            </div>
          </div>

          <div className="grid md:grid-cols-3 gap-8 max-w-6xl mx-auto">
            {/* Free Plan */}
            <div className="rounded-xl border p-8 space-y-6 bg-card flex flex-col">
              <div className="space-y-2">
                <h3 className="text-2xl font-bold">Free</h3>
                <p className="text-muted-foreground">For small communities</p>
              </div>
              <div className="text-3xl font-bold">$0 <span className="text-base font-normal text-muted-foreground">/mo</span></div>
              <ul className="space-y-3 pt-4 flex-1">
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
            <div className="rounded-xl border border-secondary/50 p-8 space-y-6 bg-card flex flex-col relative overflow-hidden">
               <div className="space-y-2">
                <h3 className="text-2xl font-bold">Plus</h3>
                <p className="text-muted-foreground">For smaller servers (1k-5k msgs)</p>
              </div>
              <div className="text-3xl font-bold">$9 <span className="text-base font-normal text-muted-foreground">/mo</span></div>
              <ul className="space-y-3 pt-4 flex-1">
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-4 w-4 text-secondary" />
                  <span>Limited Usage</span>
                </li>
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-4 w-4 text-secondary" />
                  <span>Moderation (50 actions/mo)</span>
                </li>
                 <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-4 w-4 text-secondary" />
                  <span>Extended Analytics (30 days)</span>
                </li>
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-4 w-4 text-secondary" />
                  <span>Priority Support</span>
                </li>
              </ul>
              <Button className="w-full" variant="secondary">Upgrade to Plus</Button>
            </div>

            {/* Premium Plan */}
            <div className="rounded-xl border border-primary p-8 space-y-6 bg-card flex flex-col relative overflow-hidden shadow-lg shadow-primary/10">
              <div className="absolute top-0 right-0 bg-primary text-primary-foreground px-3 py-1 text-xs font-medium rounded-bl-lg">
                POPULAR
              </div>
              <div className="space-y-2">
                <h3 className="text-2xl font-bold">Premium</h3>
                <p className="text-muted-foreground">For medium servers (5k-10k msgs)</p>
              </div>
              <div className="text-3xl font-bold">$25 <span className="text-base font-normal text-muted-foreground">/mo</span></div>
              <ul className="space-y-3 pt-4 flex-1">
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-4 w-4 text-primary" />
                  <span>Semi-limited Usage</span>
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
              <Button className="w-full">Upgrade to Premium</Button>
            </div>
          </div>

           <div className="mt-20 text-center">
            <h2 className="text-2xl font-bold mb-4">Enterprise?</h2>
            <p className="text-muted-foreground mb-8">Need more than what's listed here? Contact us for a custom plan.</p>
            <Button variant="outline">Contact Sales</Button>
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
