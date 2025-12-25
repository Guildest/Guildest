import { redirect } from "next/navigation";
import { Sidebar } from "@/components/sidebar";
import { backendFetch } from "@/lib/backend.server";
import { MeResponse } from "@/lib/types";

async function getMe(): Promise<MeResponse | null> {
  try {
    const res = await backendFetch("/me");
    if (res.status === 401) return null;
    if (!res.ok) throw new Error(`Failed to load /me (${res.status})`);
    return await res.json();
  } catch (error) {
    console.error(error);
    return null;
  }
}

export default async function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const me = await getMe();

  if (!me) {
    redirect("/");
  }

  return (
    <div className="flex min-h-screen bg-background">
      <Sidebar
        user={{
          userId: me.user_id,
          username: me.username,
          avatar: me.avatar,
          plan: me.plan,
        }}
      />
      <main className="flex-1 md:ml-64 p-8 overflow-y-auto h-screen">
        {children}
      </main>
    </div>
  );
}
