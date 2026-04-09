"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";

type DeadLetterActionsProps = {
  deadLetterEntryId: string;
  guildId: string;
  sourceStream: string;
};

export function DeadLetterActions({
  deadLetterEntryId,
  guildId,
  sourceStream,
}: DeadLetterActionsProps) {
  const router = useRouter();
  const [operatorReason, setOperatorReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  function submit(action: "replay" | "discard") {
    setError(null);
    startTransition(async () => {
      const response = await fetch(
        `/api/dashboard/guilds/${guildId}/ops/incidents/${action}`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            dead_letter_entry_id: deadLetterEntryId,
            operator_reason: operatorReason,
            source_stream: sourceStream,
          }),
        },
      );

      if (!response.ok) {
        if (response.status === 409) {
          setError("Already being handled");
        } else {
          setError(`${action === "replay" ? "Replay" : "Discard"} failed`);
        }
        return;
      }

      setOperatorReason("");
      router.refresh();
    });
  }

  return (
    <div className="flex w-[280px] flex-col items-end gap-2.5">
      <textarea
        value={operatorReason}
        onChange={(event) => setOperatorReason(event.target.value)}
        disabled={isPending}
        rows={3}
        maxLength={280}
        placeholder="Optional operator note..."
        className="min-h-[72px] w-full rounded-xl border border-border-light bg-surface px-3 py-2.5 text-xs leading-relaxed text-cream outline-none placeholder:text-cream/20 focus:border-tan/30 focus:ring-1 focus:ring-tan/20 transition-all"
      />
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => submit("replay")}
          disabled={isPending}
          className="inline-flex h-9 items-center justify-center rounded-lg bg-tan/90 px-4 text-xs font-semibold text-plum transition-all hover:bg-tan disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isPending ? "Working..." : "Replay"}
        </button>
        <button
          type="button"
          onClick={() => submit("discard")}
          disabled={isPending}
          className="inline-flex h-9 items-center justify-center rounded-lg border border-border-light bg-surface-light px-4 text-xs font-semibold text-cream/70 transition-all hover:bg-surface hover:text-cream disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isPending ? "Working..." : "Discard"}
        </button>
      </div>
      <div className="flex w-full items-center justify-between text-[10px] text-cream/25">
        <span>Reason optional</span>
        <span className="font-mono">{operatorReason.length}/280</span>
      </div>
      {error ? (
        <p className="text-xs font-medium text-red-400">
          {error}
        </p>
      ) : null}
    </div>
  );
}
