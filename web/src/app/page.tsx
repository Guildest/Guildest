import Image from "next/image";
import Link from "next/link";
import { getPublicStats } from "@/lib/public-api";
import { SiteNav } from "@/components/site-nav";
import { SubscribeForm } from "./subscribe-form";

export default async function Home() {
  const stats = await getPublicStats();
  const year = new Date().getFullYear();

  return (
    <div className="min-h-screen bg-plum text-cream">
      <SiteNav />

      {/* Hero (framed) */}
      <section className="relative">
        <div className="max-w-7xl mx-auto px-6 pt-10">
          <div className="relative border border-dashed border-border-light/30 px-6 md:px-12 py-24 md:py-36 overflow-hidden">
            <div aria-hidden className="absolute inset-0 ascii-bg pointer-events-none" />
            <div className="relative text-center">
              <h1 className="font-display text-5xl md:text-7xl leading-[1.02] tracking-tight text-cream">
                Discord communities,<br />
                guided by AI.
              </h1>
              <p className="mt-8 text-cream/55 text-base md:text-lg max-w-2xl mx-auto leading-relaxed">
                Guildest reads every message, learns the pulse of your
                community, and tells you what to do next. You build. The AI
                listens.
              </p>
              <div className="mt-10 flex items-center justify-center gap-3 flex-wrap">
                <Link
                  href="/waitlist"
                  className="bg-cream text-plum text-sm font-medium px-5 py-2.5 hover:bg-cream/90 transition-colors"
                >
                  Join waitlist
                </Link>
                <a
                  href="#how"
                  className="text-cream text-sm font-medium px-5 py-2.5 border border-border-light/40 hover:bg-surface-light/30 transition-colors"
                >
                  How it works
                </a>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Powered by */}
      <section className="border-y border-border-light/15 mt-10">
        <div className="max-w-7xl mx-auto px-6 py-7 flex items-center justify-center gap-x-12 gap-y-3 flex-wrap text-cream/40 text-[13px]">
          <span className="text-[11px] tracking-widest uppercase text-cream/35">
            Powered by
          </span>
          <span className="font-display tracking-tight">Discord</span>
          <span className="font-display tracking-tight">Anthropic</span>
          <span className="font-display tracking-tight">Vercel</span>
          <span className="font-display tracking-tight">Stripe</span>
          <span className="font-display tracking-tight">Postgres</span>
        </div>
      </section>

      {/* Stats */}
      <section className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto grid grid-cols-2 md:grid-cols-4 divide-x divide-border-light/15">
          <Stat label="Communities tracked" value={stats.servers.toLocaleString()} />
          <Stat label="Members reached" value={stats.members.toLocaleString()} />
          <Stat label="Messages indexed" value={stats.messages_tracked.toLocaleString()} />
          <Stat label="Always listening" value="24/7" />
        </div>
      </section>

      {/* How it works */}
      <section id="how" className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-24">
          <div className="max-w-2xl">
            <h2 className="font-display text-4xl md:text-5xl tracking-tight text-cream leading-[1.05]">
              From server to signal,<br />in six steps.
            </h2>
            <p className="mt-5 text-cream/55 leading-relaxed">
              No managers, no dashboards to babysit. The AI does the watching.
            </p>
          </div>

          <div className="mt-16 grid grid-cols-1 md:grid-cols-3 border border-border-light/20">
            <Step num="01" title="Connect Discord" body="One-click install. Guildest joins your server as a bot and begins indexing within seconds." image="/Cards/card-01.png" />
            <Step num="02" title="AI reads" body="Every message processed in real time — no batch jobs, no daily syncs." image="/Cards/card-02.png" />
            <Step num="03" title="AI classifies" body="Questions, feedback, support, sentiment, urgency. Continuously categorized." image="/Cards/card-03.png" />
            <Step num="04" title="Insights surface" body="Health, retention, hotspots, onboarding funnels. The metrics that matter, ranked." image="/Cards/card-04.png" />
            <Step num="05" title="Alerts ping you" body="Sentiment dips, key channels going quiet, urgent threads — surfaced the second they happen." image="/Cards/card-05.png" />
            <Step num="06" title="You act" body="Daily briefings, weekly reports, AI-drafted replies. The clearest next step, always." image="/Cards/card-06.png" />
          </div>
        </div>
      </section>

      {/* Explore (feature trio) */}
      <section id="explore" className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-24">
          <div className="max-w-2xl">
            <h2 className="font-display text-4xl md:text-5xl tracking-tight text-cream leading-[1.05]">
              Built for the people<br />who run communities.
            </h2>
          </div>

          <div className="mt-16 grid grid-cols-1 md:grid-cols-3 border border-border-light/20">
            <Feature title="Insights" body="Health, retention, hotspots, onboarding funnels. The metrics community managers actually need." />
            <Feature title="Alerts" body="Get pinged the second sentiment dips or a key channel goes quiet. Skip the rest of the noise." />
            <Feature title="AI advisor" body="Drafts replies, summarizes threads, runs your weekly community report — on autopilot." />
          </div>
        </div>
      </section>

      {/* Banner statement */}
      <section id="product" className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-32 md:py-40 text-center">
          <h2 className="font-display text-4xl md:text-7xl tracking-tight text-cream leading-[1.02] max-w-5xl mx-auto">
            Stop watching chat.<br />
            Start building.
          </h2>
          <p className="mt-8 text-cream/55 max-w-xl mx-auto leading-relaxed">
            Guildest is the layer between your community and your day. The AI
            watches every channel, so you don&apos;t have to.
          </p>
        </div>
      </section>

      {/* FAQ */}
      <section id="faq" className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-24 grid grid-cols-1 md:grid-cols-[1fr_2fr] gap-12">
          <div>
            <h2 className="font-display text-4xl md:text-5xl tracking-tight text-cream leading-[1.05]">
              Asked first.
            </h2>
          </div>
          <div className="divide-y divide-border-light/15 border-y border-border-light/15">
            <Faq q="So — what is Guildest, really?" a="Guildest is an AI co-pilot for Discord communities. It sits inside your server, watches every channel, and quietly does the work a community manager would: reading conversations, spotting questions that need answers, flagging shifts in mood, and writing you a short brief on what your community needs from you today." />
            <Faq q="Who is it for?" a="Creators, indie SaaS, startups, agencies, and anyone running a community Discord between 500 and 50,000 members." />
            <Faq q="Is my data safe?" a="We only index public channels you grant access to. Retention is configurable. We never train models on your data." />
            <Faq q="What does it cost?" a="Free during closed beta. Paid plans launch when we open up." />
            <Faq q="When can I get in?" a="Join the waitlist with Discord and tell us what you’re building. We let people in weekly." />
          </div>
        </div>
      </section>

      {/* Subscribe */}
      <section className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-24 grid grid-cols-1 md:grid-cols-2 gap-12 items-start">
          <div>
            <h2 className="font-display text-4xl md:text-5xl tracking-tight text-cream leading-[1.05]">
              Stay in the loop.
            </h2>
            <p className="mt-5 text-cream/55 leading-relaxed max-w-md">
              Updates as Guildest opens up — new features, beta cohorts, and the
              occasional behind-the-scenes note.
            </p>
          </div>
          <SubscribeForm />
        </div>
      </section>

      {/* Final CTA */}
      <section className="border-b border-border-light/15">
        <div className="max-w-7xl mx-auto px-6 py-32 text-center">
          <Image src="/logolanding.svg" alt="" width={40} height={36} className="mx-auto opacity-70" />
          <h2 className="mt-8 font-display text-4xl md:text-6xl tracking-tight text-cream leading-[1.05]">
            Send your<br />first signal.
          </h2>
          <p className="mt-6 text-cream/55 max-w-md mx-auto leading-relaxed">
            Add Guildest to your Discord and let the AI start listening.
          </p>
          <div className="mt-10 flex items-center justify-center gap-3 flex-wrap">
            <Link
              href="/waitlist"
              className="text-sm font-medium bg-cream text-plum px-5 py-3 hover:bg-cream/90 transition-colors"
            >
              Join waitlist
            </Link>
          </div>
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
              { label: "Explore", href: "#explore" },
              { label: "How it works", href: "#how" },
              { label: "FAQ", href: "#faq" },
            ]}
          />
          <FooterCol
            title="Company"
            items={[
              { label: "Feedback", href: "mailto:hi@guildest.com" },
              { label: "Subscribe", href: "/waitlist" },
            ]}
          />
          <FooterCol
            title="Account"
            items={[
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

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="px-6 py-10">
      <div className="font-display text-4xl md:text-5xl tracking-tight text-cream">
        {value}
      </div>
      <div className="mt-3 text-[12px] text-cream/45">{label}</div>
    </div>
  );
}

function Step({
  num,
  title,
  body,
  image,
}: {
  num: string;
  title: string;
  body: string;
  image?: string;
}) {
  return (
    <div className="border-b border-border-light/20 [&:nth-child(3n)]:md:border-r-0 md:border-r last:border-b-0 md:[&:nth-last-child(-n+3)]:border-b-0 p-8 flex flex-col">
      <div className="text-[12px] tracking-wide text-cream/35 font-mono">{num}</div>
      <div className="mt-6 font-display text-2xl md:text-3xl tracking-tight text-cream leading-tight">
        {title}
      </div>
      <p className="mt-4 text-[15px] text-cream/55 leading-relaxed">{body}</p>
      {image && (
        <div className="mt-8 aspect-square relative overflow-hidden bg-surface-light/[0.04]">
          <Image
            src={image}
            alt={title}
            fill
            className="object-contain"
            sizes="(max-width: 768px) 100vw, 33vw"
          />
        </div>
      )}
    </div>
  );
}

function Feature({ title, body }: { title: string; body: string }) {
  return (
    <div className="border-b md:border-b-0 md:border-r last:border-r-0 last:border-b-0 border-border-light/20 p-8">
      <div className="font-display text-xl tracking-tight text-cream">{title}</div>
      <p className="mt-3 text-sm text-cream/55 leading-relaxed">{body}</p>
    </div>
  );
}

function Faq({ q, a }: { q: string; a: string }) {
  return (
    <details className="group py-5">
      <summary className="flex items-center justify-between cursor-pointer list-none">
        <span className="text-cream/85 text-base">{q}</span>
        <span className="text-cream/35 text-lg group-open:rotate-45 transition-transform">+</span>
      </summary>
      <p className="mt-3 text-cream/55 text-sm leading-relaxed pr-8">{a}</p>
    </details>
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
