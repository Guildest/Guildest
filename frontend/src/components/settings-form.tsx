"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useFieldArray, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import * as z from "zod";
import { Loader2, Save, Trash2, Plus, AlertCircle, CheckCircle2, Clock, Shield, BarChart3, Smile, Globe, Hash, Bell, Settings, Info } from "lucide-react";
import { GuildSettings } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { updateGuildSettings } from "@/lib/api";
import { cn } from "@/lib/utils";

const warnPolicySchema = z.object({
  threshold: z.coerce.number().int().min(1),
  action: z.enum(["timeout", "ban"]),
  duration_hours: z.coerce.number().int().min(1).max(168).optional(),
});

const DEFAULT_WARN_POLICY = [
  { threshold: 3, action: "timeout" as const, duration_hours: 24 },
  { threshold: 5, action: "ban" as const },
];

const settingsSchema = z.object({
  prefix: z.string().min(1, "Prefix is required").max(5),
  language: z.string().min(2),
  timezone: z.string().min(2),
  analytics_enabled: z.boolean(),
  sentiment_enabled: z.boolean(),
  moderation_enabled: z.boolean(),
  warn_decay_days: z.coerce.number().int().min(0).max(365),
  warn_policy: z.array(warnPolicySchema),
  welcome_channel_id: z.string().optional(),
  log_channel_id: z.string().optional(),
});

type SettingsFormValues = z.infer<typeof settingsSchema>;

interface SettingsFormProps {
  initialSettings: GuildSettings;
  guildId: string;
}

const SECTIONS = [
  { id: "general", label: "General", icon: Settings, description: "Basic bot configuration" },
  { id: "features", label: "Features", icon: CheckCircle2, description: "Toggle bot modules" },
  { id: "warnings", label: "Warnings", icon: AlertCircle, description: "Auto-mod rules" },
  { id: "channels", label: "Channels", icon: Hash, description: "Notification channels" },
];

export function SettingsForm({ initialSettings, guildId }: SettingsFormProps) {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error', text: string } | null>(null);

  const { register, handleSubmit, setValue, watch, control, formState: { errors } } = useForm<SettingsFormValues>({
    resolver: zodResolver(settingsSchema) as any,
    defaultValues: {
      prefix: initialSettings.prefix,
      language: initialSettings.language,
      timezone: initialSettings.timezone,
      analytics_enabled: initialSettings.analytics_enabled,
      sentiment_enabled: initialSettings.sentiment_enabled,
      moderation_enabled: initialSettings.moderation_enabled,
      warn_decay_days: initialSettings.warn_decay_days ?? 90,
      warn_policy: (initialSettings.warn_policy && initialSettings.warn_policy.length > 0)
        ? initialSettings.warn_policy
        : DEFAULT_WARN_POLICY,
      welcome_channel_id: initialSettings.welcome_channel_id || "",
      log_channel_id: initialSettings.log_channel_id || "",
    },
  });
  const warnPolicies = watch("warn_policy");
  const warnPolicyFields = useFieldArray({ control, name: "warn_policy" });

  const onSubmit = async (data: SettingsFormValues) => {
    setLoading(true);
    setMessage(null);
    try {
      await updateGuildSettings(guildId, data);
      setMessage({ type: 'success', text: "Settings saved successfully." });
      router.refresh();
    } catch (error) {
      console.error(error);
      setMessage({ type: 'error', text: "Failed to save settings. Please try again." });
    } finally {
      setLoading(false);
    }
  };

  return (
    <form id="guild-settings-form" onSubmit={handleSubmit(onSubmit)} className="space-y-8">
      <input type="hidden" {...register("prefix")} />

      <section id="general" className="scroll-mt-24 space-y-4">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-primary/10">
            <Globe className="h-5 w-5 text-primary" />
          </div>
          <div>
            <h2 className="text-lg font-semibold">General Configuration</h2>
            <p className="text-sm text-muted-foreground">Basic settings for how the bot operates in your server.</p>
          </div>
        </div>
        <Card>
          <CardContent className="pt-6">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div className="space-y-2">
                <Label htmlFor="language" className="flex items-center gap-2">
                  Language
                  <span className="text-muted-foreground font-normal text-xs">UI Language</span>
                </Label>
                <Input
                  id="language"
                  {...register("language")}
                  placeholder="en-US"
                  className={errors.language ? "border-destructive" : ""}
                />
                {errors.language && (
                  <p className="text-xs text-destructive">{errors.language.message}</p>
                )}
                <p className="text-xs text-muted-foreground">Sets the language for bot responses in this server.</p>
              </div>
              <div className="space-y-2">
                <Label htmlFor="timezone" className="flex items-center gap-2">
                  Timezone
                  <Clock className="h-3 w-3 text-muted-foreground" />
                </Label>
                <Input
                  id="timezone"
                  {...register("timezone")}
                  placeholder="UTC"
                  className={errors.timezone ? "border-destructive" : ""}
                />
                {errors.timezone && (
                  <p className="text-xs text-destructive">{errors.timezone.message}</p>
                )}
                <p className="text-xs text-muted-foreground">Used for timestamp formatting in logs and reports.</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </section>

      <section id="features" className="scroll-mt-24 space-y-4">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-primary/10">
            <CheckCircle2 className="h-5 w-5 text-primary" />
          </div>
          <div>
            <h2 className="text-lg font-semibold">Features</h2>
            <p className="text-sm text-muted-foreground">Enable or disable specific bot modules for your server.</p>
          </div>
        </div>
        <Card>
          <CardContent className="pt-6">
            <div className="grid gap-4 md:grid-cols-3">
              <FeatureCard
                icon={<BarChart3 className="h-5 w-5" />}
                iconBg="bg-blue-500/10 text-blue-500"
                title="Analytics"
                description="Track message volume, engagement, and activity patterns over time."
                enabled={watch("analytics_enabled")}
                onChange={(checked) => setValue("analytics_enabled", checked)}
              />
              <FeatureCard
                icon={<Smile className="h-5 w-5" />}
                iconBg="bg-green-500/10 text-green-500"
                title="Sentiment Analysis"
                description="Monitor community mood and generate AI-powered sentiment reports."
                enabled={watch("sentiment_enabled")}
                onChange={(checked) => setValue("sentiment_enabled", checked)}
              />
              <FeatureCard
                icon={<Shield className="h-5 w-5" />}
                iconBg="bg-orange-500/10 text-orange-500"
                title="Moderation"
                description="Log moderation actions and enable automated warning enforcement."
                enabled={watch("moderation_enabled")}
                onChange={(checked) => setValue("moderation_enabled", checked)}
              />
            </div>
          </CardContent>
        </Card>
      </section>

      <section id="warnings" className="scroll-mt-24 space-y-4">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-primary/10">
            <AlertCircle className="h-5 w-5 text-primary" />
          </div>
          <div>
            <h2 className="text-lg font-semibold">Warnings & Auto-Mod</h2>
            <p className="text-sm text-muted-foreground">Configure automatic actions when users accumulate warnings.</p>
          </div>
        </div>
        <Card>
          <CardContent className="pt-6 space-y-6">
            <div className="space-y-2">
              <Label htmlFor="warn_decay_days" className="flex items-center gap-2">
                Warning Expiry
                <span className="text-muted-foreground font-normal text-xs">(Days)</span>
              </Label>
              <div className="flex items-center gap-3">
                <Input
                  id="warn_decay_days"
                  type="number"
                  min={0}
                  max={365}
                  {...register("warn_decay_days", { valueAsNumber: true })}
                  className="w-32"
                />
                <span className="text-sm text-muted-foreground">
                  {watch("warn_decay_days") === 0
                    ? "Warnings never expire"
                    : `Warnings expire after ${watch("warn_decay_days")} days`}
                </span>
              </div>
              <p className="text-xs text-muted-foreground">
                Set to <strong>0</strong> to disable automatic expiry. Expired warnings don&apos;t count toward auto-mod thresholds.
              </p>
            </div>

            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <Label className="text-base">Auto-Mod Rules</Label>
                  <p className="text-sm text-muted-foreground">
                    Define thresholds for automatic timeout or ban actions.
                  </p>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => warnPolicyFields.append({ threshold: 3, action: "timeout", duration_hours: 24 })}
                  className="gap-2"
                >
                  <Plus className="h-4 w-4" />
                  Add Rule
                </Button>
              </div>

              {warnPolicyFields.fields.length === 0 ? (
                <div className="rounded-lg border border-dashed p-6 text-center">
                  <div className="mx-auto w-12 h-12 rounded-full bg-muted/50 flex items-center justify-center mb-3">
                    <AlertCircle className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <p className="text-sm font-medium">No automatic punishment rules</p>
                  <p className="text-xs text-muted-foreground mt-1">
                    Users won&apos;t be automatically punished for accumulating warnings.
                  </p>
                </div>
              ) : (
                <div className="space-y-3">
                  {warnPolicyFields.fields.map((field, index) => {
                    const action = warnPolicies?.[index]?.action ?? field.action;
                    return (
                      <div key={field.id} className="rounded-lg border p-4 space-y-4 bg-muted/30">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <div className="flex items-center justify-center w-6 h-6 rounded-full bg-primary text-primary-foreground text-xs font-medium">
                              {index + 1}
                            </div>
                            <span className="text-sm font-medium">Rule {index + 1}</span>
                          </div>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => warnPolicyFields.remove(index)}
                            className="text-muted-foreground hover:text-destructive gap-1"
                          >
                            <Trash2 className="h-4 w-4" />
                            Remove
                          </Button>
                        </div>
                        <div className="grid gap-4 md:grid-cols-3">
                          <div className="space-y-2">
                            <Label className="text-xs uppercase tracking-wider text-muted-foreground">Warns</Label>
                            <Input
                              type="number"
                              min={1}
                              {...register(`warn_policy.${index}.threshold`, { valueAsNumber: true })}
                            />
                          </div>
                          <div className="space-y-2">
                            <Label className="text-xs uppercase tracking-wider text-muted-foreground">Action</Label>
                            <select
                              className="h-10 rounded-md border bg-background px-3 text-sm w-full"
                              {...register(`warn_policy.${index}.action`)}
                            >
                              <option value="timeout">Timeout</option>
                              <option value="ban">Ban</option>
                            </select>
                          </div>
                          <div className="space-y-2">
                            <Label className="text-xs uppercase tracking-wider text-muted-foreground">
                              Duration {action === "timeout" ? "(hours)" : ""}
                            </Label>
                            <Input
                              type="number"
                              min={1}
                              max={168}
                              disabled={action !== "timeout"}
                              {...register(`warn_policy.${index}.duration_hours`, { valueAsNumber: true })}
                              placeholder={action !== "timeout" ? "N/A" : undefined}
                            />
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </section>

      <section id="channels" className="scroll-mt-24 space-y-4">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-primary/10">
            <Hash className="h-5 w-5 text-primary" />
          </div>
          <div>
            <h2 className="text-lg font-semibold">Channels</h2>
            <p className="text-sm text-muted-foreground">Configure where the bot sends notifications and logs.</p>
          </div>
        </div>
        <Card>
          <CardContent className="pt-6">
            <div className="grid gap-6 md:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="welcome_channel" className="flex items-center gap-2">
                  <Bell className="h-4 w-4 text-muted-foreground" />
                  Welcome Channel
                </Label>
                <Input
                  id="welcome_channel"
                  {...register("welcome_channel_id")}
                  placeholder="123456789012345678"
                />
                <p className="text-xs text-muted-foreground">
                  Channel ID for welcome messages when new members join.
                </p>
              </div>
              <div className="space-y-2">
                <Label htmlFor="log_channel" className="flex items-center gap-2">
                  <Hash className="h-4 w-4 text-muted-foreground" />
                  Log Channel
                </Label>
                <Input
                  id="log_channel"
                  {...register("log_channel_id")}
                  placeholder="123456789012345678"
                />
                <p className="text-xs text-muted-foreground">
                  Channel ID for moderation logs and audit trail.
                </p>
              </div>
            </div>
          </CardContent>
          <CardFooter className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 border-t px-6 py-4 bg-muted/20">
            {message && (
              <div className={cn(
                "flex items-center gap-2 text-sm",
                message.type === 'success' ? "text-green-600 dark:text-green-400" : "text-destructive"
              )}>
                {message.type === 'success' ? (
                  <CheckCircle2 className="h-4 w-4" />
                ) : (
                  <AlertCircle className="h-4 w-4" />
                )}
                {message.text}
              </div>
            )}
            <Button type="submit" disabled={loading} className="gap-2 sm:ml-auto">
              {loading ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Saving...
                </>
              ) : (
                <>
                  <Save className="h-4 w-4" />
                  Save Changes
                </>
              )}
            </Button>
          </CardFooter>
        </Card>
      </section>
    </form>
  );
}

function FeatureCard({
  icon,
  iconBg,
  title,
  description,
  enabled,
  onChange,
}: {
  icon: React.ReactNode;
  iconBg: string;
  title: string;
  description: string;
  enabled: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className={cn(
      "relative rounded-xl border p-4 transition-all",
      enabled
        ? "bg-card hover:border-primary/50"
        : "bg-muted/30 hover:bg-muted/50"
    )}>
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          <div className={cn("p-2 rounded-lg", iconBg)}>
            {icon}
          </div>
          <div className="space-y-1">
            <Label className="text-base cursor-pointer" htmlFor={title.toLowerCase().replace(" ", "-")}>
              {title}
            </Label>
            <p className="text-sm text-muted-foreground leading-relaxed">
              {description}
            </p>
          </div>
        </div>
        <Switch
          id={title.toLowerCase().replace(" ", "-")}
          checked={enabled}
          onCheckedChange={onChange}
          className="shrink-0"
        />
      </div>
      <div className={cn(
        "mt-3 flex items-center gap-2 text-xs",
        enabled ? "text-green-600 dark:text-green-400" : "text-muted-foreground"
      )}>
        <div className={cn(
          "w-1.5 h-1.5 rounded-full",
          enabled ? "bg-green-500" : "bg-muted-foreground"
        )} />
        {enabled ? "Active" : "Disabled"}
      </div>
    </div>
  );
}
