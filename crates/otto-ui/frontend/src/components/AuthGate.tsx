import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import ottoMark from "@/assets/otto-mark.png";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { signInWithPat, startGithubDeviceFlow } from "@/lib/tauri";
import type { DeviceCodePayload, Profile } from "@/lib/types";
import { errorMessage } from "@/lib/utils";

type Mode = "idle" | "device-flow" | "paste-token";

/**
 * Two sign-in paths: the GitHub Device Flow (backend opens the browser
 * itself, same as the Google Calendar connect flow; this component just
 * shows the code while it waits) or pasting an existing token.
 *
 * Rendered two ways: full-screen in place of the Sidebar/Dashboard whenever
 * there's no signed-in profile at all (`App`'s `currentProfile === null` -
 * a genuine fresh install, or after `signOut`), or `embedded` inside a
 * Dialog for "Sign in another account" from the Sidebar's profile menu
 * while already signed in - `switch_to_profile` on the backend makes the
 * new sign-in current either way, so the two cases share this same flow.
 */
export function AuthGate({
  onSignedIn,
  embedded = false,
}: {
  onSignedIn: (profile: Profile) => void;
  embedded?: boolean;
}) {
  const [mode, setMode] = useState<Mode>("idle");
  const [deviceCode, setDeviceCode] = useState<DeviceCodePayload | null>(null);
  const [signingIn, setSigningIn] = useState(false);
  const [token, setToken] = useState("");

  useEffect(() => {
    const unlisten = listen<DeviceCodePayload>("device-code", (event) => setDeviceCode(event.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function handleGithubSignIn() {
    setMode("device-flow");
    setDeviceCode(null);
    setSigningIn(true);
    try {
      const profile = await startGithubDeviceFlow();
      onSignedIn(profile);
    } catch (err) {
      toast.error("GitHub sign-in failed", { description: errorMessage(err) });
      setMode("idle");
    } finally {
      setSigningIn(false);
      setDeviceCode(null);
    }
  }

  async function handleTokenSignIn() {
    const value = token.trim();
    if (!value) return;
    setSigningIn(true);
    try {
      const profile = await signInWithPat(value);
      onSignedIn(profile);
    } catch (err) {
      toast.error("Sign-in failed", { description: errorMessage(err) });
    } finally {
      setSigningIn(false);
    }
  }

  const content = (
    <>
      {!embedded && (
        <div className="flex items-center gap-2.5 text-[17px] font-bold">
          <img src={ottoMark} alt="" className="h-8 w-8 rounded-[8px]" />
          <span>Otto</span>
        </div>
      )}

      {mode === "device-flow" ? (
        <div className="flex w-[320px] flex-col items-center gap-4 text-center">
          {deviceCode ? (
            <>
              <div className="text-sm text-muted">Enter this code at</div>
              <a
                href={deviceCode.verification_uri}
                target="_blank"
                rel="noreferrer"
                className="text-sm font-semibold text-accent-2 underline"
              >
                {deviceCode.verification_uri}
              </a>
              <div className="rounded-lg border border-border bg-surface px-4 py-2 font-mono text-lg font-bold tracking-[0.3em]">
                {deviceCode.user_code}
              </div>
              <div className="text-xs text-muted">Waiting for you to finish signing in...</div>
            </>
          ) : (
            <div className="text-sm text-muted">Requesting a sign-in code...</div>
          )}
          <Button type="button" variant="ghost" onClick={() => setMode("idle")}>
            Use a different sign-in method
          </Button>
        </div>
      ) : mode === "paste-token" ? (
        <div className="flex w-[280px] flex-col gap-3">
          <Label htmlFor="pat-token">GitHub personal access token</Label>
          <Input
            id="pat-token"
            type="password"
            autoComplete="off"
            placeholder="ghp_..."
            value={token}
            onChange={(e) => setToken(e.target.value)}
          />
          <Button type="button" disabled={signingIn || !token.trim()} onClick={handleTokenSignIn}>
            {signingIn ? "Signing in..." : "Sign in"}
          </Button>
          <Button type="button" variant="ghost" onClick={() => setMode("idle")}>
            Back
          </Button>
        </div>
      ) : (
        <div className="flex w-[280px] flex-col gap-3">
          <Button type="button" onClick={handleGithubSignIn}>
            Sign in with GitHub
          </Button>
          <Button type="button" variant="ghost" onClick={() => setMode("paste-token")}>
            Paste a token instead
          </Button>
        </div>
      )}
    </>
  );

  if (embedded) {
    return <div className="flex flex-col items-center gap-6">{content}</div>;
  }

  return (
    <div className="flex h-screen min-h-0 flex-1 flex-col items-center justify-center gap-8 bg-bg text-ink antialiased">
      {content}
    </div>
  );
}
