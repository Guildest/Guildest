const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

export interface User {
  user_id: string;
  plan: "free" | "plus" | "premium";
  guilds: Guild[];
}

export interface Guild {
  id: string;
  name: string;
  icon: string | null;
  owner: boolean;
  permissions: number;
  features: string[];
  connected: boolean;
  guild_id: string; // The backend seems to use guild_id or id somewhat interchangeably in some contexts, but let's match the backend response
}

export interface GuildSettings {
  guild_id: string;
  prefix: string;
  language: string;
  timezone: string;
  analytics_enabled: boolean;
  sentiment_enabled: boolean;
  moderation_enabled: boolean;
  warn_decay_days?: number;
  warn_policy?: {
    threshold: number;
    action: "timeout" | "ban";
    duration_hours?: number;
  }[];
  welcome_channel_id?: string;
  log_channel_id?: string;
}

export async function fetchMe(): Promise<User | null> {
  try {
    const res = await fetch(`${API_BASE_URL}/me`, {
      credentials: "include", // Important for cookies
    });
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.error("Failed to fetch user:", error);
    return null;
  }
}

export async function fetchGuildSettings(guildId: string): Promise<GuildSettings | null> {
  try {
    const res = await fetch(`/api/guilds/${guildId}/settings`, {
      credentials: "include",
    });
    if (!res.ok) throw new Error("Failed to fetch settings");
    return await res.json();
  } catch (error) {
    console.error("Failed to fetch guild settings:", error);
    return null;
  }
}

export async function updateGuildSettings(guildId: string, settings: Partial<GuildSettings>) {
  const res = await fetch(`/api/guilds/${guildId}/settings`, {
    method: "PATCH",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(settings),
    credentials: "include",
  });
  if (!res.ok) throw new Error("Failed to update settings");
  return await res.json();
}

export async function connectGuild(guildId: string) {
  const res = await fetch(`/api/guilds/${guildId}/connect`, {
    method: "POST",
    credentials: "include",
  });
  if (!res.ok) {
    let detail = "Failed to connect guild";
    try {
      const data = await res.json();
      if (data?.detail) detail = String(data.detail);
    } catch {
      // ignore parse errors
    }
    throw new Error(detail);
  }
  return await res.json();
}

export async function disconnectGuild(guildId: string) {
  const res = await fetch(`/api/guilds/${guildId}/disconnect`, {
    method: "POST",
    credentials: "include",
  });
  if (!res.ok) {
    let detail = "Failed to disconnect guild";
    try {
      const data = await res.json();
      if (data?.detail) detail = String(data.detail);
    } catch {
      // ignore parse errors
    }
    throw new Error(detail);
  }
  return await res.json();
}

export async function summarizeAppeal(guildId: string, appealId: string) {
  const res = await fetch(`/api/guilds/${guildId}/appeals/${appealId}/summarize`, {
    method: "POST",
    credentials: "include",
  });
  if (!res.ok) {
    let detail = "Failed to summarize appeal";
    try {
      const data = await res.json();
      if (data?.detail) detail = String(data.detail);
    } catch {
      // ignore parse errors
    }
    throw new Error(detail);
  }
  return await res.json();
}

export async function approveAppeal(guildId: string, appealId: string) {
  const res = await fetch(`/api/guilds/${guildId}/appeals/${appealId}/unban`, {
    method: "POST",
    credentials: "include",
  });
  if (!res.ok) {
    let detail = "Failed to approve appeal";
    try {
      const data = await res.json();
      if (data?.detail) detail = String(data.detail);
    } catch {
      // ignore parse errors
    }
    throw new Error(detail);
  }
  return await res.json();
}

export async function deleteAppeal(guildId: string, appealId: string) {
  const res = await fetch(`/api/guilds/${guildId}/appeals/${appealId}/delete`, {
    method: "POST",
    credentials: "include",
  });
  if (!res.ok) {
    let detail = "Failed to delete appeal";
    try {
      const data = await res.json();
      if (data?.detail) detail = String(data.detail);
    } catch {
      // ignore parse errors
    }
    throw new Error(detail);
  }
  return await res.json();
}

export async function blockAppeal(guildId: string, appealId: string) {
  const res = await fetch(`/api/guilds/${guildId}/appeals/${appealId}/block`, {
    method: "POST",
    credentials: "include",
  });
  if (!res.ok) {
    let detail = "Failed to block appeals";
    try {
      const data = await res.json();
      if (data?.detail) detail = String(data.detail);
    } catch {
      // ignore parse errors
    }
    throw new Error(detail);
  }
  return await res.json();
}

export async function logout() {
  await fetch(`${API_BASE_URL}/auth/logout`, {
    method: "POST",
    credentials: "include",
  });
  window.location.href = "/";
}

export const LOGIN_URL = `${API_BASE_URL}/auth/discord/login`;

export function buildLoginUrl(redirectPath: string = "/dashboard"): string {
  const safe = redirectPath.startsWith("/") && !redirectPath.startsWith("//") ? redirectPath : "/dashboard";
  const qs = new URLSearchParams({ redirect: safe });
  return `${LOGIN_URL}?${qs.toString()}`;
}

export async function createBillingCheckoutUrl(plan: "plus" | "premium" = "plus"): Promise<string> {
  const res = await fetch(`/api/billing/checkout`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ plan }),
    credentials: "include",
  });
  if (res.status === 401) throw new Error("unauthenticated");
  if (!res.ok) throw new Error(`Failed to create checkout session (${res.status})`);
  const data = await res.json();
  if (!data?.url) throw new Error("Missing checkout url");
  return data.url;
}

export async function createBillingPortalUrl(): Promise<string> {
  const res = await fetch(`/api/billing/portal`, {
    method: "POST",
    credentials: "include",
  });
  if (res.status === 401) throw new Error("unauthenticated");
  if (!res.ok) throw new Error(`Failed to create portal session (${res.status})`);
  const data = await res.json();
  if (!data?.url) throw new Error("Missing portal url");
  return data.url;
}
