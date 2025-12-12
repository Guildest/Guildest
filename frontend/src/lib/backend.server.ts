import "server-only";

import { cookies } from "next/headers";

export const API_BASE = (process.env.API_BASE ?? "http://localhost:8000").replace(/\/$/, "");

export async function getSessionToken(): Promise<string | undefined> {
  return (await cookies()).get("guildest_session")?.value;
}

export async function backendFetch(path: string, init?: RequestInit): Promise<Response> {
  const token = await getSessionToken();
  const url = `${API_BASE}${path.startsWith("/") ? "" : "/"}${path}`;

  const headers = new Headers(init?.headers);
  if (token) headers.set("authorization", `Bearer ${token}`);

  return fetch(url, {
    ...init,
    headers,
    cache: "no-store",
  });
}
