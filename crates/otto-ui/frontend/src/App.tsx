import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { ActivitiesView } from "@/components/views/ActivitiesView";
import { DashboardView } from "@/components/views/DashboardView";
import { ModelsView } from "@/components/views/ModelsView";
import { SkillsView } from "@/components/views/SkillsView";
import { Sidebar } from "@/components/Sidebar";
import { SettingsDialog } from "@/components/SettingsDialog";
import { Toaster } from "@/components/ui/sonner";
import { TopBar } from "@/components/TopBar";
import { useOttoStatus } from "@/hooks/useOttoStatus";
import type { Tab } from "@/lib/tabs";

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>("dashboard");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { status, refresh, providerName } = useOttoStatus();

  useEffect(() => {
    refresh().then((result) => {
      const selectedKey = `${result.selected?.provider}-api-key`;
      const modelConfigured = result.credentialStatus.find((s) => s.key === selectedKey)?.configured ?? true;
      const githubConfigured = result.credentialStatus.find((s) => s.key === "github-token")?.configured;
      if (!modelConfigured || !githubConfigured) {
        toast.info("Finish setting up Otto", {
          description: !modelConfigured
            ? "Add an API key for your selected model's provider in Settings."
            : "Add a GitHub token in Settings to enable the GitHub skill.",
          duration: 8000,
        });
      }
    });
  }, [refresh]);

  useEffect(() => {
    const unlisten = listen("open-settings", () => setSettingsOpen(true));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

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

  return (
    <div className="flex h-screen min-h-0 flex-1 bg-bg text-ink antialiased">
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        onSettingsClick={() => setSettingsOpen(true)}
        modelLabel={modelLabel}
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
      <Toaster position="top-right" />
    </div>
  );
}
