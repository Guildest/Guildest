"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { buildLoginUrl, createBillingCheckoutUrl } from "@/lib/api";

type Props = {
  redirectAfterLogin?: string;
  label?: string;
  variant?: "default" | "secondary" | "outline" | "ghost";
};

export function CheckoutProButton({
  redirectAfterLogin = "/pricing?checkout=pro",
  label = "Upgrade to Pro",
  variant = "default",
}: Props) {
  const [loading, setLoading] = useState(false);

  async function onClick() {
    try {
      setLoading(true);
      const url = await createBillingCheckoutUrl("pro");
      window.location.href = url;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message === "unauthenticated") {
        window.location.href = buildLoginUrl(redirectAfterLogin);
        return;
      }
      alert(message);
    } finally {
      setLoading(false);
    }
  }

  return (
    <Button className="w-full" variant={variant} onClick={onClick} disabled={loading}>
      {loading ? "Redirecting..." : label}
    </Button>
  );
}

