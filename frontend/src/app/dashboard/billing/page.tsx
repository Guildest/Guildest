import { redirect } from "next/navigation";
import { backendFetch } from "@/lib/backend.server";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { BillingButtons } from "@/components/billing-buttons";

async function getBilling() {
  const res = await backendFetch("/billing/subscription");
  if (res.status === 401) return null;
  if (!res.ok) throw new Error(`Failed to load billing subscription (${res.status})`);
  return res.json();
}

export default async function BillingPage() {
  const billing = await getBilling();
  if (!billing) redirect("/");

  return (
    <div className="space-y-8 max-w-3xl mx-auto">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Billing</h1>
        <p className="text-muted-foreground">Manage your subscription.</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Current plan</CardTitle>
          <CardDescription>
            Plan: <span className="font-medium text-foreground">{billing.plan}</span>
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="text-sm text-muted-foreground">
            Status: <span className="text-foreground">{billing.status}</span>
            {billing.current_period_end ? (
              <>
                {" "}
                · Renews/ends: <span className="text-foreground">{billing.current_period_end}</span>
              </>
            ) : null}
          </div>
          <BillingButtons plan={billing.plan} />
        </CardContent>
      </Card>
    </div>
  );
}

