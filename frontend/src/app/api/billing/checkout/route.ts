import { NextResponse } from "next/server";
import { backendFetch } from "@/lib/backend.server";

async function forward(res: Response) {
  const body = await res.text();
  const contentType = res.headers.get("content-type") ?? "application/json";
  return new NextResponse(body, { status: res.status, headers: { "content-type": contentType } });
}

export async function POST(req: Request) {
  let payload: unknown;
  try {
    payload = await req.json();
  } catch {
    return NextResponse.json({ error: "Invalid JSON" }, { status: 400 });
  }

  const res = await backendFetch("/billing/checkout", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  return forward(res);
}
