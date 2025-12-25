"use client";

import { useEffect, useRef } from "react";
import { buildLoginUrl, createBillingCheckoutUrl } from "@/lib/api";

type Props = {
  enabled: boolean;
  plan?: "plus" | "premium";
  redirectAfterLogin?: string;
};

export function AutoStartPlanCheckout({ enabled, plan = "plus", redirectAfterLogin }: Props) {
  const started = useRef(false);

  useEffect(() => {
    if (!enabled || started.current) return;
    started.current = true;

    (async () => {
      try {
        const url = await createBillingCheckoutUrl(plan);
        window.location.href = url;
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        if (message === "unauthenticated") {
          const redirect = redirectAfterLogin || `/pricing?checkout=${plan}`;
          window.location.href = buildLoginUrl(redirect);
          return;
        }
        alert(message);
      }
    })();
  }, [enabled, plan, redirectAfterLogin]);

  return null;
}
