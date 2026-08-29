import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { connectGoogleCalendar, getPreferences, setCredential, setPreferences } from "@/lib/tauri";
import type { CredentialStatus, Preferences } from "@/lib/types";
import { cn, errorMessage } from "@/lib/utils";

// Not a plain paste field like every other credential - written only by the
// OAuth connect flow itself (see `connectGoogleCalendar`).
const GOOGLE_CALENDAR_OAUTH_KEY = "google-calendar-oauth";
// Set by signing in (see `AuthGate`/the Sidebar profile menu), not pasted
// here - excluded from the credentials list below.
const GITHUB_KEY = "github-token";

const DEFAULT_PREFERENCES: Preferences = {
  persona: "",
  talk_back_default: false,
  notifications_enabled: true,
};

type SettingsTab = "credentials" | "preferences";

export function SettingsDialog({
  open,
  onOpenChange,
  credentialStatus,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  credentialStatus: CredentialStatus[];
  onSaved: () => Promise<unknown>;
}) {
  const [tab, setTab] = useState<SettingsTab>("credentials");
  const [values, setValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [preferences, setPreferencesState] = useState<Preferences>(DEFAULT_PREFERENCES);
  const [savingPreferences, setSavingPreferences] = useState(false);

  useEffect(() => {
    if (!open) return;
    getPreferences()
      .then(setPreferencesState)
      .catch((err) => toast.error("Couldn't load preferences", { description: errorMessage(err) }));
  }, [open]);

  // Credentials are OS-keychain-backed and never sent back to the frontend,
  // so fields always start blank - clearing on close (rather than reacting
  // to `open` becoming true) means they're already blank by the time the
  // dialog reopens, with no effect needed to synchronize the two.
  function handleOpenChange(next: boolean) {
    if (!next) {
      setValues({});
      setTab("credentials");
    }
    onOpenChange(next);
  }

  async function handleConnect() {
    setConnecting(true);
    try {
      await connectGoogleCalendar();
      toast.success("Google Calendar connected");
      await onSaved();
    } catch (err) {
      toast.error("Couldn't connect Google Calendar", { description: errorMessage(err) });
    } finally {
      setConnecting(false);
    }
  }

  async function handleSave() {
    const dirty = Object.entries(values).filter(([, value]) => value.trim());
    if (dirty.length === 0) {
      handleOpenChange(false);
      return;
    }

    setSaving(true);
    try {
      for (const [key, value] of dirty) {
        await setCredential(key, value.trim());
      }
      toast.success("Settings saved", { description: "Otto picked up the new credentials immediately." });
      handleOpenChange(false);
      await onSaved();
    } catch (err) {
      toast.error("Couldn't save settings", { description: errorMessage(err) });
    } finally {
      setSaving(false);
    }
  }

  async function handleSavePreferences() {
    setSavingPreferences(true);
    try {
      await setPreferences(preferences);
      toast.success("Preferences saved");
      handleOpenChange(false);
    } catch (err) {
      toast.error("Couldn't save preferences", { description: errorMessage(err) });
    } finally {
      setSavingPreferences(false);
    }
  }

  const visibleCredentials = credentialStatus.filter((cred) => cred.key !== GITHUB_KEY);

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="flex max-h-[80vh] w-[440px] max-w-[440px] flex-col gap-0 overflow-hidden rounded-2xl p-5.5 shadow-modal">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>

        <div className="mb-3 flex gap-1 rounded-lg bg-surface-sunken p-1 text-xs font-semibold">
          {(["credentials", "preferences"] as const).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTab(t)}
              className={cn(
                "flex-1 rounded-md px-2 py-1.5 capitalize transition-colors",
                tab === t ? "bg-surface text-ink shadow-sm" : "text-muted hover:text-ink",
              )}
            >
              {t}
            </button>
          ))}
        </div>

        {tab === "credentials" ? (
          <>
            <div className="mb-3 text-xs text-muted">
              Credentials are stored in your OS keychain - never in a plaintext file. Leave a field blank to keep its
              current value. GitHub is connected from the Sidebar's profile menu, not here.
            </div>

            <div className="flex flex-col gap-3 overflow-y-auto pr-0.5">
              {visibleCredentials.map((cred) => (
                <div key={cred.key}>
                  <div className="mb-1.5 flex justify-between text-xs font-semibold">
                    <Label htmlFor={`cred-${cred.key}`}>{cred.label}</Label>
                    <span className={cred.configured ? "font-medium text-success" : "font-medium text-muted"}>
                      {cred.configured ? "configured" : "not set"}
                    </span>
                  </div>
                  {cred.key === GOOGLE_CALENDAR_OAUTH_KEY ? (
                    <Button type="button" variant="secondary" className="w-full" disabled={connecting} onClick={handleConnect}>
                      {connecting ? "Connecting..." : cred.configured ? "Reconnect" : "Connect"}
                    </Button>
                  ) : (
                    <Input
                      id={`cred-${cred.key}`}
                      type="password"
                      autoComplete="off"
                      placeholder="Paste key/token..."
                      className="font-mono text-[13px]"
                      value={values[cred.key] ?? ""}
                      onChange={(e) => setValues((prev) => ({ ...prev, [cred.key]: e.target.value }))}
                    />
                  )}
                </div>
              ))}
            </div>

            <div className="mt-4 flex justify-end gap-2">
              <Button type="button" variant="secondary" onClick={() => handleOpenChange(false)}>
                Close
              </Button>
              <Button
                type="button"
                className="bg-navy text-white hover:bg-navy/90"
                disabled={saving}
                onClick={handleSave}
              >
                Save
              </Button>
            </div>
          </>
        ) : (
          <>
            <div className="flex flex-col gap-4 overflow-y-auto pr-0.5">
              <div>
                <Label htmlFor="persona">Persona</Label>
                <textarea
                  id="persona"
                  rows={4}
                  placeholder="How should Otto talk to you? Leave blank for the default persona."
                  className="mt-1.5 w-full resize-none rounded-lg border border-input bg-transparent px-2.5 py-1.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
                  value={preferences.persona}
                  onChange={(e) => setPreferencesState((prev) => ({ ...prev, persona: e.target.value }))}
                />
              </div>

              <div className="flex items-center justify-between">
                <Label htmlFor="talk-back-default">Talk back by default</Label>
                <Switch
                  id="talk-back-default"
                  checked={preferences.talk_back_default}
                  onCheckedChange={(checked) =>
                    setPreferencesState((prev) => ({ ...prev, talk_back_default: checked }))
                  }
                />
              </div>

              <div className="flex items-center justify-between">
                <Label htmlFor="notifications-enabled">Desktop notifications</Label>
                <Switch
                  id="notifications-enabled"
                  checked={preferences.notifications_enabled}
                  onCheckedChange={(checked) =>
                    setPreferencesState((prev) => ({ ...prev, notifications_enabled: checked }))
                  }
                />
              </div>
            </div>

            <div className="mt-4 flex justify-end gap-2">
              <Button type="button" variant="secondary" onClick={() => handleOpenChange(false)}>
                Close
              </Button>
              <Button
                type="button"
                className="bg-navy text-white hover:bg-navy/90"
                disabled={savingPreferences}
                onClick={handleSavePreferences}
              >
                Save
              </Button>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
