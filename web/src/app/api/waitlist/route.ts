import { NextResponse } from "next/server";

export async function POST(request: Request) {
  const body = await request.json().catch(() => null);
  if (!body) {
    return NextResponse.json({ error: "invalid body" }, { status: 400 });
  }

  // TODO: forward to api crate once /v1/public/waitlist exists.
  // For now, log so submissions during the waitlist push are captured.
  console.log("[waitlist]", JSON.stringify(body));

  return NextResponse.json({ ok: true });
}
