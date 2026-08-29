import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { AuthGate } from "@/components/AuthGate";
import { ActivitiesView } from "@/components/views/ActivitiesView";
import { DashboardView } from "@/components/views/DashboardView";
import { ModelsView } from "@/components/views/ModelsView";
import { SkillsView } from "@/components/views/SkillsView";
import { Sidebar } from "@/components/Sidebar";
import { SettingsDialog } from "@/components/SettingsDialog";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Toaster } from "@/components/ui/sonner";
import { TopBar } from "@/components/TopBar";
import { useOttoStatus } from "@/hooks/useOttoStatus";
import { getCurrentProfile, listProfiles, signOut, switchProfile } from "@/lib/tauri";
import type { Tab } from "@/lib/tabs";
import type { Profile } from "@/lib/types";
import { errorMessage } from "@/lib/utils";

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>("dashboard");
  const [settingsOpen, setSettingsOpen] = useState(false);
  // `undefined` = still checking who's signed in; `null` = nobody is (the
  // sign-in gate shows). Distinct from `null` so the gate doesn't flash
  // before the initial `getCurrentProfile` call resolves.
  const [currentProfile, setCurrentProfile] = useState<Profile | null | undefined>(undefined);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [addAccountOpen, setAddAccountOpen] = useState(false);
  const { status, refresh, providerName } = useOttoStatus();

  const refreshProfiles = useCallback(async () => {
    const list = await listProfiles();
    setProfiles(list);
    return list;
  }, []);

  useEffect(() => {
    getCurrentProfile()
      .then(setCurrentProfile)
      .catch(() => setCurrentProfile(null));
    refreshProfiles().catch(() => {});
  }, [refreshProfiles]);

  useEffect(() => {
    if (!currentProfile) return;
    refresh().then((result) => {
      const selectedKey = `${result.selected?.provider}-api-key`;
      const modelConfigured = result.credentialStatus.find((s) => s.key === selectedKey)?.configured ?? true;
      const githubConfigured = result.credentialStatus.find((s) => s.key === "github-token")?.configured;
      if (!modelConfigured || !githubConfigured) {
        toast.info("Finish setting up Otto", {
          description: !modelConfigured
            ? "Add an API key for your selected model's provider in Settings."
            : "Sign in with GitHub from the Sidebar's profile menu to enable the GitHub skill.",
          duration: 8000,
        });
      }
    });
  }, [currentProfile, refresh]);

  useEffect(() => {
    const unlisten = listen("open-settings", () => setSettingsOpen(true));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function handleSignedIn(profile: Profile) {
    setCurrentProfile(profile);
    setAddAccountOpen(false);
    await Promise.all([refreshProfiles(), refresh()]);
  }

  async function handleSwitchProfile(profileId: string) {
    if (profileId === currentProfile?.id) return;
    try {
      await switchProfile(profileId);
      setCurrentProfile(await getCurrentProfile());
      await refresh();
    } catch (err) {
      toast.error("Couldn't switch profile", { description: errorMessage(err) });
    }
  }

  async function handleSignOut() {
    try {
      await signOut();
      setCurrentProfile(null);
    } catch (err) {
      toast.error("Couldn't sign out", { description: errorMessage(err) });
    }
  }

  const modelPill = useMemo(() => {
    if (!status.selected) return { ok: false, label: "Model" };
    const providerInfo = status.providers.find((p) => p.id === status.selected!.provider);
    const ok =
      !providerInfo?.needs_api_key ||
      !!status.credentialStatus.find((s) => s.key === `${status.selected!.provider}-api-key`)?.configured;
    return { ok, label: providerInfo?.name ?? status.selected.provider };
  }, [status]);

  const githubPill = useMemo(() => {
    const github = status.credentialStatus.find((s) => s.key === "github-token");
    return { ok: !!github?.configured, label: "GitHub" };
  }, [status.credentialStatus]);

  const modelLabel = status.selected ? `${providerName(status.selected.provider)} · ${status.selected.model}` : "Choose a model";

  if (currentProfile === undefined) {
    // Still checking who's signed in - nothing to show yet, and showing
    // either the gate or the dashboard here would just flash and flip.
    return <div className="h-screen bg-bg" />;
  }

  if (currentProfile === null) {
    return (
      <>
        <AuthGate onSignedIn={handleSignedIn} />
        <Toaster position="top-right" />
      </>
    );
  }

  return (
    <div className="flex h-screen min-h-0 flex-1 bg-bg text-ink antialiased">
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        onSettingsClick={() => setSettingsOpen(true)}
        modelLabel={modelLabel}
        currentProfile={currentProfile}
        profiles={profiles}
        onSwitchProfile={handleSwitchProfile}
        onSignInAnother={() => setAddAccountOpen(true)}
        onSignOut={handleSignOut}
      />

      <div className="flex min-h-0 flex-1 flex-col">
        <TopBar activeTab={activeTab} modelPill={modelPill} githubPill={githubPill} />

        {/* Dashboard stays mounted across tab switches so the transcript and
            PR panel survive navigating away and back - the other tabs have
            no state worth preserving and simply refetch on every mount,
            matching the original "refetch every switch" behavior for free. */}
        <DashboardView active={activeTab === "dashboard"} providerName={providerName} />
        {activeTab === "activities" && <ActivitiesView providerName={providerName} />}
        {activeTab === "models" && (
          <ModelsView status={status} refreshStatus={refresh} onOpenSettings={() => setSettingsOpen(true)} />
        )}
        {activeTab === "skills" && <SkillsView />}
      </div>

      <SettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        credentialStatus={status.credentialStatus}
        onSaved={refresh}
      />

      <Dialog open={addAccountOpen} onOpenChange={setAddAccountOpen}>
        <DialogContent className="w-[380px] max-w-[380px] rounded-2xl p-6">
          <DialogHeader>
            <DialogTitle>Sign in another account</DialogTitle>
          </DialogHeader>
          <AuthGate embedded onSignedIn={handleSignedIn} />
        </DialogContent>
      </Dialog>

      <Toaster position="top-right" />
    </div>
  );
}
