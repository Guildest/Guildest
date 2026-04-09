"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import { LayoutDashboard, Users, Flame, Bot } from "lucide-react";

export function DashboardNav({ guildId }: { guildId?: string }) {
  const pathname = usePathname();

  const getHref = (path: string) => {
    if (!guildId) return path;
    return `${path}?guild_id=${guildId}`;
  };

  const links = [
    {
      href: "/dashboard",
      label: "Overview",
      icon: LayoutDashboard,
      active: pathname === "/dashboard",
    },
    {
      href: "/dashboard/hotspots",
      label: "Hotspots",
      icon: Flame,
      active: pathname === "/dashboard/hotspots",
    },
    {
      href: "/dashboard/users",
      label: "Users",
      icon: Users,
      active: pathname === "/dashboard/users",
    },
    {
      href: "/dashboard/ai",
      label: "AI",
      icon: Bot,
      active: pathname === "/dashboard/ai",
    },
  ];

  return (
    <nav className="flex items-center gap-1 rounded-2xl bg-surface p-1.5 w-fit border border-border">
      {links.map((link) => {
        const Icon = link.icon;
        return (
          <Link
            key={link.href}
            href={getHref(link.href)}
            className={`flex items-center gap-2 rounded-xl px-4 py-2 text-sm font-medium transition-all ${
              link.active
                ? "bg-tan/15 text-tan shadow-sm"
                : "text-cream/50 hover:text-cream hover:bg-surface-light"
            }`}
          >
            <Icon className="h-4 w-4" />
            {link.label}
          </Link>
        );
      })}
    </nav>
  );
}
