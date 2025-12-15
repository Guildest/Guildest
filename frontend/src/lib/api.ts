const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

export interface User {
  user_id: string;
  plan: "free" | "pro";
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
    const res = await fetch(`${API_BASE_URL}/guilds/${guildId}/settings`, {
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
  const res = await fetch(`${API_BASE_URL}/guilds/${guildId}/settings`, {
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
  const res = await fetch(`${API_BASE_URL}/guilds/${guildId}/connect`, {
    method: "POST",
    credentials: "include",
  });
  if (!res.ok) throw new Error("Failed to connect guild");
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

export async function createBillingCheckoutUrl(plan: "pro" = "pro"): Promise<string> {
  const res = await fetch(`${API_BASE_URL}/billing/checkout`, {
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
  const res = await fetch(`${API_BASE_URL}/billing/portal`, {
    method: "POST",
    credentials: "include",
  });
  if (res.status === 401) throw new Error("unauthenticated");
  if (!res.ok) throw new Error(`Failed to create portal session (${res.status})`);
  const data = await res.json();
  if (!data?.url) throw new Error("Missing portal url");
  return data.url;
}
