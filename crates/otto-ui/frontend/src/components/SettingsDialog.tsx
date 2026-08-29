import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { setCredential } from "@/lib/tauri";
import type { CredentialStatus } from "@/lib/types";
import { errorMessage } from "@/lib/utils";

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
  const [values, setValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);

  // Credentials are OS-keychain-backed and never sent back to the frontend,
  // so fields always start blank - clearing on close (rather than reacting
  // to `open` becoming true) means they're already blank by the time the
  // dialog reopens, with no effect needed to synchronize the two.
  function handleOpenChange(next: boolean) {
    if (!next) setValues({});
    onOpenChange(next);
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

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="flex max-h-[80vh] w-[440px] max-w-[440px] flex-col gap-0 overflow-hidden rounded-2xl p-5.5 shadow-modal">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>
        <div className="mb-3 text-xs text-muted">
          Credentials are stored in your OS keychain - never in a plaintext file. Leave a field blank to keep its
          current value.
        </div>

        <div className="flex flex-col gap-3 overflow-y-auto pr-0.5">
          {credentialStatus.map((cred) => (
            <div key={cred.key}>
              <div className="mb-1.5 flex justify-between text-xs font-semibold">
                <Label htmlFor={`cred-${cred.key}`}>{cred.label}</Label>
                <span className={cred.configured ? "font-medium text-success" : "font-medium text-muted-2"}>
                  {cred.configured ? "configured" : "not set"}
                </span>
              </div>
              <Input
                id={`cred-${cred.key}`}
                type="password"
                autoComplete="off"
                placeholder="Paste key/token..."
                className="font-mono text-[13px]"
                value={values[cred.key] ?? ""}
                onChange={(e) => setValues((prev) => ({ ...prev, [cred.key]: e.target.value }))}
              />
            </div>
          ))}
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <Button type="button" variant="secondary" onClick={() => handleOpenChange(false)}>
            Close
          </Button>
          <Button type="button" className="bg-navy text-white hover:bg-navy/90" disabled={saving} onClick={handleSave}>
            Save
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
