"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import * as z from "zod";
import { Loader2, Save } from "lucide-react";
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
import { toast } from "sonner"; // Assuming we might add a toast library, or I'll simple alert for now if not installed.

// I'll install sonner or use simple alert. Let's stick to simple state feedback for now to avoid extra deps if not requested, 
// but sonner is standard in modern stacks. I'll add it to the install list or just use state.
// Actually, I'll use simple state for success/error.

const settingsSchema = z.object({
  prefix: z.string().min(1, "Prefix is required").max(5),
  language: z.string().min(2),
  timezone: z.string().min(2),
  analytics_enabled: z.boolean(),
  sentiment_enabled: z.boolean(),
  moderation_enabled: z.boolean(),
  welcome_channel_id: z.string().optional(),
  log_channel_id: z.string().optional(),
});

type SettingsFormValues = z.infer<typeof settingsSchema>;

interface SettingsFormProps {
  initialSettings: GuildSettings;
  guildId: string;
}

export function SettingsForm({ initialSettings, guildId }: SettingsFormProps) {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error', text: string } | null>(null);

  const { register, handleSubmit, setValue, watch, formState: { errors } } = useForm<SettingsFormValues>({
    resolver: zodResolver(settingsSchema),
    defaultValues: {
      prefix: initialSettings.prefix,
      language: initialSettings.language,
      timezone: initialSettings.timezone,
      analytics_enabled: initialSettings.analytics_enabled,
      sentiment_enabled: initialSettings.sentiment_enabled,
      moderation_enabled: initialSettings.moderation_enabled,
      welcome_channel_id: initialSettings.welcome_channel_id || "",
      log_channel_id: initialSettings.log_channel_id || "",
    },
  });

  const onSubmit = async (data: SettingsFormValues) => {
    setLoading(true);
    setMessage(null);
    try {
      await updateGuildSettings(guildId, data);
      setMessage({ type: 'success', text: "Settings saved successfully." });
      router.refresh();
    } catch (error) {
      console.error(error);
      setMessage({ type: 'error', text: "Failed to save settings." });
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>General Configuration</CardTitle>
          <CardDescription>
            Basic settings for the bot in your server.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-2">
            <Label htmlFor="prefix">Command Prefix</Label>
            <Input id="prefix" {...register("prefix")} placeholder="!" />
            {errors.prefix && (
              <p className="text-sm text-destructive">{errors.prefix.message}</p>
            )}
          </div>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="grid gap-2">
              <Label htmlFor="language">Language</Label>
              <Input id="language" {...register("language")} placeholder="en-US" />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="timezone">Timezone</Label>
              <Input id="timezone" {...register("timezone")} placeholder="UTC" />
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Features</CardTitle>
          <CardDescription>
            Enable or disable specific bot modules.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between rounded-lg border p-4">
            <div className="space-y-0.5">
              <Label className="text-base">Analytics</Label>
              <p className="text-sm text-muted-foreground">
                Track message volume and activity stats.
              </p>
            </div>
            <Switch
              checked={watch("analytics_enabled")}
              onCheckedChange={(checked) => setValue("analytics_enabled", checked)}
            />
          </div>
          <div className="flex items-center justify-between rounded-lg border p-4">
            <div className="space-y-0.5">
              <Label className="text-base">Sentiment Analysis</Label>
              <p className="text-sm text-muted-foreground">
                Analyze community mood and generate reports.
              </p>
            </div>
            <Switch
              checked={watch("sentiment_enabled")}
              onCheckedChange={(checked) => setValue("sentiment_enabled", checked)}
            />
          </div>
          <div className="flex items-center justify-between rounded-lg border p-4">
            <div className="space-y-0.5">
              <Label className="text-base">Moderation</Label>
              <p className="text-sm text-muted-foreground">
                Log moderation actions and enable auto-mod.
              </p>
            </div>
            <Switch
              checked={watch("moderation_enabled")}
              onCheckedChange={(checked) => setValue("moderation_enabled", checked)}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Channels</CardTitle>
          <CardDescription>
            Configure where the bot sends notifications.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-2">
            <Label htmlFor="welcome_channel">Welcome Channel ID</Label>
            <Input id="welcome_channel" {...register("welcome_channel_id")} placeholder="123456789012345678" />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="log_channel">Log Channel ID</Label>
            <Input id="log_channel" {...register("log_channel_id")} placeholder="123456789012345678" />
          </div>
        </CardContent>
        <CardFooter className="flex justify-between border-t px-6 py-4">
           {message && (
            <p className={message.type === 'success' ? "text-green-600" : "text-destructive"}>
              {message.text}
            </p>
          )}
          <Button type="submit" disabled={loading} className="ml-auto gap-2">
            {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
            Save Changes
          </Button>
        </CardFooter>
      </Card>
    </form>
  );
}
