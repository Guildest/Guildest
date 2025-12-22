"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { buildLoginUrl, createBillingCheckoutUrl, createBillingPortalUrl } from "@/lib/api";

type Props = {
  plan: string;
};

export function BillingButtons({ plan }: Props) {
  const [loading, setLoading] = useState<"checkout" | "portal" | null>(null);

  async function onUpgradeToPlus() {
    try {
      setLoading("checkout");
      const url = await createBillingCheckoutUrl("plus");
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

  async function onUpgradeToPremium() {
    try {
      setLoading("checkout");
      const url = await createBillingCheckoutUrl("premium");
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

  const hasPaidPlan = plan === "plus" || plan === "premium";

  return (
    <div className="flex flex-col sm:flex-row gap-3">
      {!hasPaidPlan ? (
        <>
          <Button onClick={onUpgradeToPlus} disabled={loading !== null}>
            {loading === "checkout" ? "Redirecting..." : "Upgrade to Plus"}
          </Button>
          <Button variant="outline" onClick={onUpgradeToPremium} disabled={loading !== null}>
            {loading === "checkout" ? "Redirecting..." : "Upgrade to Premium"}
          </Button>
        </>
      ) : plan === "plus" ? (
        <>
          <Button onClick={onUpgradeToPremium} disabled={loading !== null}>
            {loading === "checkout" ? "Redirecting..." : "Upgrade to Premium"}
          </Button>
          <Button variant="secondary" onClick={onManage} disabled={loading !== null}>
            {loading === "portal" ? "Redirecting..." : "Manage subscription"}
          </Button>
        </>
      ) : (
        <Button variant="secondary" onClick={onManage} disabled={loading !== null}>
          {loading === "portal" ? "Redirecting..." : "Manage subscription"}
        </Button>
      )}
    </div>
  );
}

