import { NextResponse } from "next/server";

function getApiBaseUrl() {
  return process.env.GUILDEST_API_BASE_URL ?? "http://127.0.0.1:8080";
}

export async function POST(request: Request) {
  const body = await request.json().catch(() => null);
  if (!body || typeof body.email !== "string") {
    return NextResponse.json({ error: "email required" }, { status: 400 });
  }

  const userAgent = request.headers.get("user-agent") ?? "";

  try {
    const upstream = await fetch(`${getApiBaseUrl()}/v1/public/subscribe`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "User-Agent": userAgent,
      },
      body: JSON.stringify({ email: body.email }),
      cache: "no-store",
    });

    if (!upstream.ok) {
      const text = await upstream.text();
      console.error("[subscribe] upstream failed", upstream.status, text);
      return NextResponse.json(
        { error: "upstream failed" },
        { status: upstream.status === 400 ? 400 : 502 },
      );
    }

    const data = await upstream.json();
    return NextResponse.json(data);
  } catch (err) {
    console.error("[subscribe] network error", err);
    return NextResponse.json({ error: "network error" }, { status: 502 });
  }
}
