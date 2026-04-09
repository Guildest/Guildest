import { cookies } from "next/headers";

type RouteContext = {
  params: Promise<{
    guildId: string;
  }>;
};

function getApiBaseUrl() {
  return process.env.GUILDEST_API_BASE_URL ?? "http://127.0.0.1:8080";
}

export async function POST(request: Request, context: RouteContext) {
  const { guildId } = await context.params;
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();
  const body = await request.text();

  const response = await fetch(
    `${getApiBaseUrl()}/v1/dashboard/guilds/${guildId}/ops/incidents/discard`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(cookieHeader ? { Cookie: cookieHeader } : {}),
      },
      body,
      cache: "no-store",
    },
  );

  return new Response(await response.text(), {
    status: response.status,
    headers: {
      "content-type":
        response.headers.get("content-type") ?? "application/json; charset=utf-8",
    },
  });
}
