import { NextResponse } from "next/server";
import { backendFetch } from "@/lib/backend.server";

async function forward(res: Response) {
  const body = await res.text();
  const contentType = res.headers.get("content-type") ?? "application/json";
  return new NextResponse(body, { status: res.status, headers: { "content-type": contentType } });
}

export async function POST(
  _req: Request,
  { params }: { params: Promise<{ guildId: string }> }
) {
  const { guildId } = await params;
  const res = await backendFetch(`/guilds/${guildId}/connect`, { method: "POST" });
  return forward(res);
}
