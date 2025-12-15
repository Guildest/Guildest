"use client";

import { useEffect, useRef } from "react";
import { buildLoginUrl, createBillingCheckoutUrl } from "@/lib/api";

type Props = {
  enabled: boolean;
  redirectAfterLogin?: string;
};

export function AutoStartProCheckout({ enabled, redirectAfterLogin = "/pricing?checkout=pro" }: Props) {
  const started = useRef(false);

  useEffect(() => {
    if (!enabled || started.current) return;
    started.current = true;

    (async () => {
      try {
        const url = await createBillingCheckoutUrl("pro");
        window.location.href = url;
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        if (message === "unauthenticated") {
          window.location.href = buildLoginUrl(redirectAfterLogin);
          return;
        }
        alert(message);
      }
    })();
  }, [enabled, redirectAfterLogin]);

  return null;
}

