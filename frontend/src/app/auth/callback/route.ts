import { NextRequest, NextResponse } from "next/server";

function safeRedirectPath(path: string | null): string {
  if (!path) return "/dashboard";
  if (!path.startsWith("/")) return "/dashboard";
  if (path.startsWith("//")) return "/dashboard";
  return path;
}

export async function GET(req: NextRequest) {
  const url = new URL(req.url);
  const token = url.searchParams.get("token");
  const redirect = safeRedirectPath(url.searchParams.get("redirect"));

  if (!token) {
    return NextResponse.redirect(new URL("/", url.origin));
  }

  const response = NextResponse.redirect(new URL(redirect, url.origin));
  response.cookies.set("guildest_session", token, {
    httpOnly: true,
    sameSite: "lax",
    secure: false,
    path: "/",
    maxAge: 60 * 60 * 24 * 7,
  });
  return response;
}

