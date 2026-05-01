import Image from "next/image";
import { SiteNav } from "@/components/site-nav";
import { TeamsLeadForm } from "./teams-lead-form";

export const metadata = {
  title: "Guildest for Teams — Voice of Customer for Discord communities",
  description:
    "Turn your Discord into the clearest product signal your team has. Voice of Customer, support triage, and real-time launch monitoring — built for startups and enterprise.",
};

export default function TeamsPage() {
  const year = new Date().getFullYear();

  return (
    <div className="min-h-screen bg-plum text-cream">
      <SiteNav />

      {/* Hero */}
      <section className="relative">
        <div className="max-w-7xl mx-auto px-6 pt-10">
          <div className="relative border border-dashed border-border-light/30 px-6 md:px-12 py-24 md:py-32 overflow-hidden">
            <div aria-hidden className="absolute inset-0 ascii-bg pointer-events-none" />
            <div className="relative text-center">
              <h1 className="font-display text-5xl md:text-7xl leading-[1.02] tracking-tight text-cream">
                Your Discord<br />is your roadmap.
              </h1>
              <p className="mt-8 text-cream/55 text-base md:text-lg max-w-2xl mx-auto leading-relaxed">
                Every feature request, complaint, and bug report your users
                shout into chat — extracted, tagged, and shipped straight to the
                people who can act on it.
              </p>
              <div className="mt-10 flex items-center justify-center gap-3 flex-wrap">
                <a
                  href="#contact"
                  className="bg-cream text-plum text-sm font-medium px-5 py-2.5 hover:bg-cream/90 transition-colors"
                >
                  Book a demo
                </a>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Logo strip */}
      <section className="border-y border-border-light/15 mt-10">
        <div className="max-w-7xl mx-auto px-6 py-7 flex items-center justify-center gap-x-12 gap-y-3 flex-wrap text-cream/40 text-[13px]">
          <span className="text-[11px] tracking-widest uppercase text-cream/35">
            Built for teams at
          </span>
          <span className="font-display tracking-tight">YC startups</span>
          <span className="font-display tracking-tight">Indie SaaS</span>
          <span className="font-display tracking-tight">Creator tools</span>
          <span className="font-display tracking-tight">DevTools</span>
          <span className="font-display tracking-tight">Web3</span>
        </div>
      </section>

      {/* The pain */}
      <section className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-24">
          <div className="max-w-3xl">
            <h2 className="font-display text-4xl md:text-5xl tracking-tight text-cream leading-[1.05]">
              Your users are telling you<br />exactly what to build.
            </h2>
            <p className="mt-6 text-cream/55 leading-relaxed text-lg">
              They&apos;re also telling you what&apos;s broken, what they&apos;d
              pay for, who&apos;s about to churn, and which launch landed.
              You&apos;re missing 95% of it because no one can read every
              message in every channel every day.
            </p>
            <p className="mt-4 text-cream/55 leading-relaxed text-lg">
              Guildest can.
            </p>
          </div>
        </div>
      </section>

      {/* Use cases */}
      <section id="use-cases" className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-24">
          <div className="max-w-2xl">
            <h2 className="font-display text-4xl md:text-5xl tracking-tight text-cream leading-[1.05]">
              Three signals,<br />always on.
            </h2>
          </div>

          <div className="mt-16 grid grid-cols-1 md:grid-cols-3 border border-border-light/20">
            <UseCase
              num="01"
              title="Voice of Customer"
              body="Every feature request, bug report, and complaint extracted from chat, tagged, and synced to Linear, Notion, or Slack. Stop reading 4,000 messages a week to find the 12 that matter."
              points={[
                "Auto-tag by product area",
                "Dedupe and rank by frequency",
                "One-click export to your tracker",
                "Weekly VoC digest to product",
              ]}
            />
            <UseCase
              num="02"
              title="Support triage"
              body="Every support question classified, routed, or auto-answered. Your CM team handles 3× the volume. Your users get answers in minutes, not hours."
              points={[
                "Real-time intent classification",
                "Auto-route by topic and urgency",
                "AI-drafted replies for review",
                "SLA tracking and reports",
              ]}
            />
            <UseCase
              num="03"
              title="Launch monitoring"
              body="Ship a feature, watch real-time sentiment in #general, get pinged the second things turn. Faster rollback, sharper launches, fewer surprises."
              points={[
                "Live sentiment by channel",
                "Anomaly alerts on launch days",
                "Cohort-level reaction tracking",
                "Pre-launch baseline comparison",
              ]}
            />
          </div>
        </div>
      </section>

      {/* Removal of "how teams use it" section */}

      {/* Outcomes */}
      <section id="security" className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-24">
          <div className="max-w-2xl">
            <h2 className="font-display text-4xl md:text-5xl tracking-tight text-cream leading-[1.05]">
              Numbers teams<br />actually move.
            </h2>
            <p className="mt-5 text-cream/55 leading-relaxed">
              The kind of impact that ends up in board decks and quarterly
              reviews.
            </p>
          </div>

          <div className="mt-12 grid grid-cols-1 md:grid-cols-3 border border-border-light/20">
            <Outcome
              metric="8h"
              label="Saved per CM, per week"
              body="Hours your community team spends scrolling, tagging, and triaging — gone. Spent on the work that actually moves the community instead."
            />
            <Outcome
              metric="99%"
              label="Of feedback caught"
              body="Voice-of-Customer extraction surfaces nearly every actionable signal across every channel. Your PMs stop guessing what users want."
            />
            <Outcome
              metric="10×"
              label="Coverage per CM"
              body="One community manager with Guildest watches as many channels as a ten-person team reading by hand. Headcount goes further."
            />
            <Outcome
              metric="40%"
              label="Auto-deflection rate"
              body="Common questions answered by the AI before a human ever sees them. CMs focus on the nuanced ones."
            />
            <Outcome
              metric="90s"
              label="Sentiment-shift alert"
              body="Ship a feature, watch real-time sentiment, get pinged the second something turns. Faster rollback, calmer launches."
            />
            <Outcome
              metric="<60s"
              label="Time to first insight"
              body="Guildest joins your server, indexes recent history, and surfaces the first actionable signal in under a minute. No onboarding call needed."
            />
          </div>
        </div>
      </section>

      {/* Final CTA — book a demo */}
      <section id="contact" className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-24 grid grid-cols-1 md:grid-cols-2 gap-12 items-start">
          <div>
            <h2 className="font-display text-4xl md:text-5xl tracking-tight text-cream leading-[1.05]">
              See it on your<br />own community.
            </h2>
            <p className="mt-5 text-cream/55 leading-relaxed max-w-md">
              30-minute demo. We&apos;ll connect Guildest to a sandbox of your
              server, show you live VoC extraction, and answer security
              questions on the spot.
            </p>
          </div>
          <TeamsLeadForm />
        </div>
      </section>

      {/* Footer */}
      <footer>
        <div className="max-w-7xl mx-auto px-6 py-14 grid grid-cols-2 md:grid-cols-5 gap-8 text-[13px]">
          <div className="col-span-2 md:col-span-2">
            <Image src="/logolanding.svg" alt="Guildest" width={28} height={26} />
            <p className="mt-4 text-cream/40 max-w-xs leading-relaxed">
              The AI layer for Discord communities.
            </p>
          </div>
          <FooterCol
            title="Product"
            items={[
              { label: "Home", href: "/" },
              { label: "For teams", href: "/teams" },
              { label: "Use cases", href: "#use-cases" },
            ]}
          />
          <FooterCol
            title="Company"
            items={[
              { label: "Outcomes", href: "#security" },
              { label: "Feedback", href: "mailto:hi@guildest.com" },
            ]}
          />
          <FooterCol
            title="Account"
            items={[
              { label: "Book a demo", href: "#contact" },
              { label: "Join waitlist", href: "/waitlist" },
            ]}
          />
        </div>
        <div className="border-t border-border-light/15">
          <div className="max-w-7xl mx-auto px-6 py-6 flex items-center justify-between text-[12px] text-cream/30">
            <span>© {year} Guildest. All rights reserved.</span>
            <span>Built quietly.</span>
          </div>
        </div>
      </footer>
    </div>
  );
}

function UseCase({
  num,
  title,
  body,
  points,
}: {
  num: string;
  title: string;
  body: string;
  points: string[];
}) {
  return (
    <div className="border-b md:border-b-0 md:border-r last:border-r-0 last:border-b-0 border-border-light/20 p-8 flex flex-col">
      <div className="text-[12px] tracking-wide text-cream/35 font-mono">{num}</div>
      <div className="mt-6 font-display text-2xl md:text-3xl tracking-tight text-cream leading-tight">
        {title}
      </div>
      <p className="mt-4 text-[15px] text-cream/55 leading-relaxed">{body}</p>
      <ul className="mt-6 space-y-2 text-sm text-cream/65">
        {points.map((p) => (
          <li key={p} className="flex gap-3">
            <span className="text-cream/30">—</span>
            <span>{p}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Outcome({
  metric,
  label,
  body,
}: {
  metric: string;
  label: string;
  body: string;
}) {
  return (
    <div className="border-b border-border-light/20 [&:nth-child(3n)]:md:border-r-0 md:border-r last:border-b-0 md:[&:nth-last-child(-n+3)]:border-b-0 p-8 flex flex-col">
      <div className="font-display text-5xl md:text-6xl tracking-tight text-cream leading-none">
        {metric}
      </div>
      <div className="mt-3 text-[12px] tracking-widest uppercase text-cream/40">
        {label}
      </div>
      <p className="mt-5 text-sm text-cream/55 leading-relaxed">{body}</p>
    </div>
  );
}

function FooterCol({
  title,
  items,
}: {
  title: string;
  items: Array<{ label: string; href: string }>;
}) {
  return (
    <div>
      <div className="text-[11px] tracking-widest uppercase text-cream/30 mb-4">{title}</div>
      <ul className="space-y-2">
        {items.map((item) => (
          <li key={item.label}>
            <a href={item.href} className="text-cream/55 hover:text-cream transition-colors">
              {item.label}
            </a>
          </li>
        ))}
      </ul>
    </div>
  );
}
