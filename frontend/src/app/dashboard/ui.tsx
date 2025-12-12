"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

type Props =
  | { kind: "connect"; guildId: string }
  | { kind: "logout" };

export function DashboardClient(props: Props) {
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  if (props.kind === "logout") {
    return (
      <button
        className="inline-flex h-9 items-center justify-center rounded-lg border px-3 text-xs font-medium hover:bg-zinc-50 disabled:opacity-50"
        disabled={busy}
        onClick={async () => {
          setBusy(true);
          try {
            await fetch("/api/backend/auth/logout", { method: "POST" });
            document.cookie = "guildest_session=; Max-Age=0; path=/";
            router.push("/");
            router.refresh();
          } finally {
            setBusy(false);
          }
        }}
      >
        Log out
      </button>
    );
  }

  return (
    <button
      className="inline-flex h-9 items-center justify-center rounded-lg border px-3 text-xs font-medium hover:bg-zinc-50 disabled:opacity-50"
      disabled={busy}
      onClick={async () => {
        setBusy(true);
        try {
          const res = await fetch(`/api/backend/guilds/${props.guildId}/connect`, { method: "POST" });
          if (!res.ok) {
            const msg = await res.text();
            throw new Error(msg || `Failed (${res.status})`);
          }
          router.refresh();
        } catch (e) {
          console.error(e);
          alert("Failed to connect guild. Ensure you have Manage Guild permissions.");
        } finally {
          setBusy(false);
        }
      }}
    >
      Connect
    </button>
  );
}

