"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { buildLoginUrl, createBillingCheckoutUrl, createBillingPortalUrl } from "@/lib/api";

type Props = {
  plan: string;
};

export function BillingButtons({ plan }: Props) {
  const [loading, setLoading] = useState<"checkout" | "portal" | null>(null);

  async function onUpgrade() {
    try {
      setLoading("checkout");
      const url = await createBillingCheckoutUrl("pro");
      window.location.href = url;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message === "unauthenticated") {
        window.location.href = buildLoginUrl("/dashboard/billing");
        return;
      }
      alert(message);
    } finally {
      setLoading(null);
    }
  }

  async function onManage() {
    try {
      setLoading("portal");
      const url = await createBillingPortalUrl();
      window.location.href = url;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message === "unauthenticated") {
        window.location.href = buildLoginUrl("/dashboard/billing");
        return;
      }
      alert(message);
    } finally {
      setLoading(null);
    }
  }

  const isPro = plan === "pro";

  return (
    <div className="flex flex-col sm:flex-row gap-3">
      {!isPro ? (
        <Button onClick={onUpgrade} disabled={loading !== null}>
          {loading === "checkout" ? "Redirecting..." : "Upgrade to Pro"}
        </Button>
      ) : (
        <Button variant="secondary" onClick={onManage} disabled={loading !== null}>
          {loading === "portal" ? "Redirecting..." : "Manage subscription"}
        </Button>
      )}
    </div>
  );
}

