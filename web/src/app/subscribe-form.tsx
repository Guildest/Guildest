"use client";

import { useState } from "react";

export function SubscribeForm() {
  const [email, setEmail] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (submitting) return;

    setSubmitting(true);
    setError(null);

    try {
      const res = await fetch("/api/subscribe", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email: email.trim() }),
      });

      if (!res.ok) {
        setError("Please double-check your email and try again.");
        return;
      }

      setSubmitted(true);
    } catch {
      setError("Network error. Try again in a sec.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="self-end">
      <form onSubmit={handleSubmit} className="flex items-stretch gap-2">
        <input
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="you@domain.com"
          required
          className="flex-1 bg-transparent border border-border-light/30 px-4 py-3 text-sm text-cream placeholder:text-cream/30 focus:outline-none focus:border-cream/50"
        />
        <button
          type="submit"
          disabled={submitting || submitted}
          className="text-sm font-medium bg-cream text-plum px-5 py-3 hover:bg-cream/90 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {submitted ? "Subscribed" : submitting ? "Sending..." : "Subscribe"}
        </button>
      </form>
      {error && <p className="mt-3 text-sm text-red-400/80">{error}</p>}
    </div>
  );
}
