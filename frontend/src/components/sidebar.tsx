"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { 
  LayoutDashboard, 
  BarChart3, 
  ShieldAlert,
  FileText,
  Settings, 
  CreditCard,
  LogOut,
  Menu,
  X
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { logout } from "@/lib/api";

interface SidebarProps {
  guildId?: string;
  user?: {
    userId?: string;
    username?: string | null;
    avatar?: string | null;
    plan?: string | null;
  };
}

export function Sidebar({ guildId: propGuildId, user }: SidebarProps) {
  const pathname = usePathname();
  const [isOpen, setIsOpen] = useState(false);

  // Extract guildId from path if not provided
  // Assumes route structure /dashboard/[guildId]/...
  const pathSegments = pathname?.split("/") || [];
  const candidateGuildId = pathSegments[1] === "dashboard" && pathSegments.length > 2 ? pathSegments[2] : undefined;
  const derivedGuildId = candidateGuildId && candidateGuildId !== "billing" ? candidateGuildId : undefined;
  
  const guildId = propGuildId || derivedGuildId;
  const displayName = user?.username || user?.userId || "User";
  const avatarUrl =
    user?.avatar && user?.userId
      ? `https://cdn.discordapp.com/avatars/${user.userId}/${user.avatar}.png?size=96`
      : null;

  // Close sidebar on route change on mobile
  useEffect(() => {
    setIsOpen(false);
  }, [pathname]);

  const billingRoute = {
    href: "/dashboard/billing",
    label: "Billing",
    icon: CreditCard,
    active: pathname === "/dashboard/billing",
  };

  const routes = guildId
    ? [
        {
          href: `/dashboard/${guildId}`,
          label: "Overview",
          icon: LayoutDashboard,
          active: pathname === `/dashboard/${guildId}`,
        },
        {
          href: `/dashboard/${guildId}/analytics`,
          label: "Analytics",
          icon: BarChart3,
          active: pathname === `/dashboard/${guildId}/analytics`,
        },
        {
          href: `/dashboard/${guildId}/moderation`,
          label: "Moderation",
          icon: ShieldAlert,
          active: pathname === `/dashboard/${guildId}/moderation`,
        },
        {
          href: `/dashboard/${guildId}/appeals`,
          label: "Appeals",
          icon: FileText,
          active: pathname === `/dashboard/${guildId}/appeals`,
        },
        {
          href: `/dashboard/${guildId}/settings`,
          label: "Settings",
          icon: Settings,
          active: pathname === `/dashboard/${guildId}/settings`,
        },
        billingRoute,
      ]
    : [
        {
          href: "/dashboard",
          label: "My Guilds",
          icon: LayoutDashboard,
          active: pathname === "/dashboard",
        },
        billingRoute,
      ];

  return (
    <>
      <Button
        variant="ghost"
        size="icon"
        className="fixed left-4 top-4 z-50 md:hidden"
        onClick={() => setIsOpen(!isOpen)}
      >
        {isOpen ? <X className="h-6 w-6" /> : <Menu className="h-6 w-6" />}
      </Button>

      <div
        className={cn(
          "fixed inset-y-0 left-0 z-40 w-64 transform border-r bg-background transition-transform duration-200 ease-in-out md:translate-x-0",
          isOpen ? "translate-x-0" : "-translate-x-full"
        )}
      >
        <div className="flex h-full flex-col">
          <div className="flex h-16 items-center border-b px-6">
            <Link href="/dashboard" className="flex items-center gap-2 font-bold text-xl">
              <span>Guildest</span>
            </Link>
          </div>
          {user && (
            <div className="border-b px-6 py-4">
              <div className="flex items-center gap-3">
                {avatarUrl ? (
                  <img
                    src={avatarUrl}
                    alt={displayName}
                    className="h-10 w-10 rounded-full border border-primary/30"
                  />
                ) : (
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-secondary text-sm font-semibold text-secondary-foreground">
                    {displayName.charAt(0).toUpperCase()}
                  </div>
                )}
                <div className="min-w-0">
                  <p className="truncate text-sm font-semibold">{displayName}</p>
                  <p className="text-xs uppercase tracking-wide text-muted-foreground">
                    {user.plan || "free"}
                  </p>
                </div>
              </div>
            </div>
          )}

          <div className="flex-1 overflow-y-auto py-4">
            <nav className="space-y-1 px-2">
              {routes.map((route) => (
                <Link
                  key={route.href}
                  href={route.href}
                  className={cn(
                    "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                    route.active
                      ? "bg-primary/10 text-primary"
                      : "text-muted-foreground hover:bg-muted hover:text-foreground"
                  )}
                >
                  <route.icon className="h-4 w-4" />
                  {route.label}
                </Link>
              ))}
            </nav>
          </div>

          <div className="border-t p-4">
            <Button
              variant="outline"
              className="w-full justify-start gap-2"
              onClick={() => logout()}
            >
              <LogOut className="h-4 w-4" />
              Logout
            </Button>
          </div>
        </div>
      </div>
    </>
  );
}
