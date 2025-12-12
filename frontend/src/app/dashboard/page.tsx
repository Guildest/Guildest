import Link from "next/link";
import { redirect } from "next/navigation";

import { backendFetch } from "@/lib/backend.server";
import type { MeResponse } from "@/lib/types";

import { DashboardClient } from "./ui";

async function getMe(): Promise<MeResponse> {
  const res = await backendFetch("/me");
  if (res.status === 401) redirect("/");
  if (!res.ok) throw new Error(`Failed to load /me (${res.status})`);
  return res.json();
}

export default async function DashboardPage() {
  const me = await getMe();

  return (
    <div className="min-h-screen bg-zinc-50 text-zinc-950">
      <main className="mx-auto flex max-w-5xl flex-col gap-8 px-6 py-10">
        <header className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-4">
            <h1 className="text-2xl font-semibold tracking-tight">Dashboard</h1>
            <DashboardClient kind="logout" />
          </div>
          <p className="text-sm text-zinc-600">
            Signed in as <span className="font-medium">{me.user_id}</span> • Plan:{" "}
            <span className="font-medium">{me.plan}</span>
          </p>
        </header>

        <section className="rounded-2xl border bg-white p-6">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold">Your servers</h2>
            <Link className="text-sm font-medium text-zinc-900 underline" href="/">
              Home
            </Link>
          </div>

          <div className="mt-5 grid gap-3">
            {me.guilds.length === 0 ? (
              <div className="text-sm text-zinc-600">No guilds found from Discord OAuth.</div>
            ) : (
              me.guilds.map((g) => (
                <div key={g.guild_id} className="flex flex-col gap-3 rounded-xl border p-4 sm:flex-row sm:items-center">
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">{g.name ?? g.guild_id}</div>
                    <div className="mt-1 text-xs text-zinc-600">Guild ID: {g.guild_id}</div>
                  </div>
                  <div className="flex items-center gap-2">
                    {g.connected ? (
                      <Link
                        className="inline-flex h-9 items-center justify-center rounded-lg bg-zinc-900 px-3 text-xs font-medium text-white hover:bg-zinc-800"
                        href={`/guilds/${g.guild_id}`}
                      >
                        Open
                      </Link>
                    ) : (
                      <DashboardClient kind="connect" guildId={g.guild_id} />
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
        </section>
      </main>
    </div>
  );
}

