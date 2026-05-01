"use client";

import { useState } from "react";

export function TeamsLeadForm() {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [company, setCompany] = useState("");
  const [message, setMessage] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (submitting) return;
    if (!email.includes("@")) {
      setError("Please enter a valid work email.");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const res = await fetch("/api/teams-lead", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          email: email.trim(),
          name: name.trim() || null,
          company: company.trim() || null,
          message: message.trim() || null,
        }),
      });
      if (!res.ok) {
        if (res.status === 400) {
          setError("Please double-check your email and try again.");
        } else {
          setError("Something went wrong. Try again in a sec.");
        }
        return;
      }
      setSubmitted(true);
    } catch {
      setError("Network error. Try again in a sec.");
    } finally {
      setSubmitting(false);
    }
  }

  if (submitted) {
    return (
      <div className="border border-border-light/30 p-8">
        <div className="flex items-center gap-3 mb-3">
          <div className="w-2 h-2 rounded-full bg-cream animate-pulse" />
          <span className="text-[11px] text-cream/60 tracking-widest uppercase">
            Got it
          </span>
        </div>
        <h3 className="font-display text-2xl tracking-tight text-cream">
          Talk soon, {name || "there"}.
        </h3>
        <p className="mt-3 text-cream/55 leading-relaxed">
          We&apos;ll reply within one business day to set up the demo.
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Your name"
        className="w-full bg-transparent border border-border-light/30 px-4 py-3 text-sm text-cream placeholder:text-cream/30 focus:outline-none focus:border-cream/50"
      />
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Work email"
        required
        className="w-full bg-transparent border border-border-light/30 px-4 py-3 text-sm text-cream placeholder:text-cream/30 focus:outline-none focus:border-cream/50"
      />
      <input
        type="text"
        value={company}
        onChange={(e) => setCompany(e.target.value)}
        placeholder="Company"
        className="w-full bg-transparent border border-border-light/30 px-4 py-3 text-sm text-cream placeholder:text-cream/30 focus:outline-none focus:border-cream/50"
      />
      <textarea
        value={message}
        onChange={(e) => setMessage(e.target.value)}
        rows={3}
        placeholder="Server size, biggest pain, anything we should know"
        className="w-full bg-transparent border border-border-light/30 px-4 py-3 text-sm text-cream placeholder:text-cream/30 focus:outline-none focus:border-cream/50 resize-none"
      />
      {error && <p className="text-sm text-red-400/80">{error}</p>}
      <button
        type="submit"
        disabled={submitting}
        className="w-full text-sm font-medium bg-cream text-plum px-5 py-3 hover:bg-cream/90 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
      >
        {submitting ? "Sending..." : "Book a demo"}
      </button>
      <p className="text-[12px] text-cream/35">
        We reply within one business day.
      </p>
    </form>
  );
}
