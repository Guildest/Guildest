"use client";

import Image from "next/image";
import { useState } from "react";

type Source =
  | "twitter"
  | "tiktok"
  | "instagram"
  | "youtube"
  | "friend"
  | "discord"
  | "search"
  | "other";

type UseCase =
  | "small_community"
  | "creator"
  | "startup"
  | "enterprise"
  | "agency"
  | "building_servers"
  | "other";

const SOURCES: Array<{ value: Source; label: string }> = [
  { value: "twitter", label: "X / Twitter" },
  { value: "tiktok", label: "TikTok" },
  { value: "instagram", label: "Instagram" },
  { value: "youtube", label: "YouTube" },
  { value: "friend", label: "A friend" },
  { value: "discord", label: "Discord" },
  { value: "search", label: "Search" },
  { value: "other", label: "Other" },
];

const USE_CASES: Array<{ value: UseCase; label: string; hint: string }> = [
  { value: "small_community", label: "Small community", hint: "Friends, hobby, side project" },
  { value: "creator", label: "Creator community", hint: "Audience, fans, members" },
  { value: "startup", label: "Startup community", hint: "Users, early customers" },
  { value: "enterprise", label: "Enterprise / brand", hint: "Large org, official server" },
  { value: "agency", label: "Agency / manager", hint: "I run servers for others" },
  { value: "building_servers", label: "Building servers", hint: "Designing new Discords" },
  { value: "other", label: "Something else", hint: "Tell us below" },
];

export function WaitlistForm({
  displayName,
  discordUserId,
}: {
  displayName: string;
  discordUserId: string;
}) {
  const [source, setSource] = useState<Source | null>(null);
  const [useCase, setUseCase] = useState<UseCase | null>(null);
  const [notes, setNotes] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSubmit = source && useCase && !submitting;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      const res = await fetch("/api/waitlist", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          discord_user_id: discordUserId,
          source,
          use_case: useCase,
          notes: notes.trim() || null,
        }),
      });
      if (!res.ok) throw new Error(`failed: ${res.status}`);
      setSubmitted(true);
    } catch {
      setError("Something went wrong. Try again in a sec.");
    } finally {
      setSubmitting(false);
    }
  }

  if (submitted) {
    return (
      <div className="border border-border-light/30 p-8 md:p-12">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-2 h-2 rounded-full bg-cream animate-pulse" />
          <span className="text-[11px] text-cream/60 tracking-widest uppercase">You&apos;re in</span>
        </div>
        <h2 className="font-display text-3xl md:text-4xl tracking-tight text-cream leading-tight">
          See you soon,<br />{displayName}.
        </h2>
        <p className="mt-5 text-cream/55 leading-relaxed max-w-md">
          We&apos;ll email you when your spot opens. Until then — keep
          building.
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-10">
      <div className="flex items-center gap-3 text-[12px] text-cream/45 border border-border-light/20 px-4 py-3">
        <Image src="/discord.svg" alt="" width={16} height={16} />
        <span>Signed in as <span className="text-cream/85">{displayName}</span></span>
      </div>

      <Field label="Where did you hear about us?">
        <div className="flex flex-wrap gap-2">
          {SOURCES.map((s) => (
            <Chip
              key={s.value}
              active={source === s.value}
              onClick={() => setSource(s.value)}
            >
              {s.label}
            </Chip>
          ))}
        </div>
      </Field>

      <Field label="What will you use Guildest for?">
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
          {USE_CASES.map((u) => (
            <button
              type="button"
              key={u.value}
              onClick={() => setUseCase(u.value)}
              className={`text-left border px-4 py-4 transition-colors ${
                useCase === u.value
                  ? "bg-cream text-plum border-cream"
                  : "bg-transparent border-border-light/30 text-cream/75 hover:border-border-light/60 hover:bg-surface-light/[0.04]"
              }`}
            >
              <div className="text-sm font-medium">{u.label}</div>
              <div className={`text-[12px] mt-0.5 ${useCase === u.value ? "text-plum/60" : "text-cream/40"}`}>{u.hint}</div>
            </button>
          ))}
        </div>
      </Field>

      <Field label="Anything else? (optional)">
        <textarea
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          rows={4}
          placeholder="Server size, what you wish Discord did, anything..."
          className="w-full bg-transparent border border-border-light/30 px-4 py-3 text-sm text-cream placeholder:text-cream/25 focus:outline-none focus:border-cream/50 resize-none"
        />
      </Field>

      {error && <p className="text-sm text-red-400/80">{error}</p>}

      <div className="flex items-center gap-3 flex-wrap">
        <button
          type="submit"
          disabled={!canSubmit}
          className="inline-flex items-center justify-center gap-2 bg-cream text-plum text-sm font-medium px-5 py-3 hover:bg-cream/90 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          <span>{submitting ? "Joining..." : "Join the waitlist"}</span>
          {!submitting && <Image src="/arrow.svg" alt="" width={16} height={16} />}
        </button>
        <span className="text-[12px] text-cream/35">We let people in weekly.</span>
      </div>
    </form>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="block text-[11px] text-cream/40 tracking-widest uppercase mb-3">
        {label}
      </label>
      {children}
    </div>
  );
}

function Chip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`px-4 py-2 text-sm transition-colors border ${
        active
          ? "bg-cream text-plum border-cream"
          : "bg-transparent border-border-light/30 text-cream/75 hover:border-border-light/60 hover:text-cream"
      }`}
    >
      {children}
    </button>
  );
}
