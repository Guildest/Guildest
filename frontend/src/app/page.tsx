export default function Home() {
  const backend = (process.env.NEXT_PUBLIC_BACKEND_URL ?? "http://localhost:8000").replace(/\/$/, "");

  return (
    <div className="min-h-screen bg-zinc-50 text-zinc-950">
      <main className="mx-auto flex max-w-5xl flex-col gap-10 px-6 py-14">
        <header className="flex flex-col gap-2">
          <h1 className="text-3xl font-semibold tracking-tight">Guildest Dashboard</h1>
          <p className="max-w-2xl text-zinc-600">
            Analytics, sentiment reports, and moderation history per Discord server (with subscription-gated features).
          </p>
        </header>

        <section className="rounded-2xl border bg-white p-6">
          <h2 className="text-lg font-semibold">Sign in</h2>
          <p className="mt-1 text-sm text-zinc-600">Use Discord OAuth to view and manage your connected servers.</p>
          <div className="mt-4 flex flex-col gap-3 sm:flex-row">
            <a
              className="inline-flex h-11 items-center justify-center rounded-xl bg-zinc-900 px-4 text-sm font-medium text-white hover:bg-zinc-800"
              href={`${backend}/auth/discord/login?redirect=/dashboard`}
            >
              Continue with Discord
            </a>
            <a
              className="inline-flex h-11 items-center justify-center rounded-xl border px-4 text-sm font-medium hover:bg-zinc-50"
              href="/dashboard"
            >
              Go to dashboard
            </a>
          </div>
        </section>

        <section className="grid gap-4 sm:grid-cols-3">
          <div className="rounded-2xl border bg-white p-5">
            <div className="text-sm font-medium">Analytics</div>
            <div className="mt-1 text-sm text-zinc-600">Message volume charts per server.</div>
          </div>
          <div className="rounded-2xl border bg-white p-5">
            <div className="text-sm font-medium">Sentiment</div>
            <div className="mt-1 text-sm text-zinc-600">Daily mood trends and (Pro) detailed AI reports.</div>
          </div>
          <div className="rounded-2xl border bg-white p-5">
            <div className="text-sm font-medium">Moderation</div>
            <div className="mt-1 text-sm text-zinc-600">Heuristic checks and (Pro) per-server audit history.</div>
          </div>
        </section>
      </main>
    </div>
  );
}

