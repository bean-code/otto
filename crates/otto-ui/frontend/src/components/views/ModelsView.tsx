import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Icon, type IconName } from "@/components/icons";
import type { OttoStatus } from "@/hooks/useOttoStatus";
import { listModels, setSelectedModel } from "@/lib/tauri";
import type { ModelInfo, ModelTier } from "@/lib/types";
import { cn, errorMessage } from "@/lib/utils";

const TIER_TONE: Record<ModelTier, { icon: string; badge: string }> = {
  flagship: { icon: "bg-accent-soft text-accent", badge: "bg-accent-soft text-accent" },
  balanced: { icon: "bg-accent-2-soft text-accent-2", badge: "bg-accent-2-soft text-accent-2" },
  fast: { icon: "bg-success-soft text-success", badge: "bg-success-soft text-success" },
  frontier: { icon: "bg-navy-soft text-navy", badge: "bg-navy-soft text-navy" },
};

const TIER_ICON: Record<ModelTier, IconName> = {
  flagship: "sparkle",
  balanced: "layers",
  fast: "bolt",
  frontier: "globe",
};

function ModelCard({ model, selected, onSelect }: { model: ModelInfo; selected: boolean; onSelect: () => void }) {
  const tone = TIER_TONE[model.tier] ?? TIER_TONE.balanced;
  return (
    <section
      onClick={onSelect}
      className={cn(
        "flex cursor-pointer flex-col gap-2.5 rounded-2xl border bg-surface p-4 shadow-card transition-all hover:-translate-y-0.5 hover:shadow-elevated",
        selected ? "border-accent-2 ring-2 ring-accent-2-soft" : "border-border",
      )}
    >
      <div className={cn("flex h-9 w-9 items-center justify-center rounded-[10px]", tone.icon)}>
        <Icon name={TIER_ICON[model.tier] ?? "layers"} className="h-5 w-5" />
      </div>
      <span className={cn("self-start rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wide", tone.badge)}>
        {model.tier}
      </span>
      <h3 className="m-0 text-sm font-bold">{model.name}</h3>
      <p className="m-0 flex-1 text-[12.5px] leading-relaxed text-muted">{model.tagline}</p>
      <div className="mt-0.5 flex items-center justify-between">
        <code className="text-[11px] text-muted-2">{model.id}</code>
        {selected && <strong className="text-[11.5px] text-accent-2">Selected</strong>}
      </div>
    </section>
  );
}

export function ModelsView({
  status,
  refreshStatus,
  onOpenSettings,
}: {
  status: OttoStatus;
  refreshStatus: () => Promise<unknown>;
  onOpenSettings: () => void;
}) {
  const [models, setModels] = useState<ModelInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [customValues, setCustomValues] = useState<Record<string, string>>({});

  useEffect(() => {
    listModels()
      .then(setModels)
      .catch((err) => setError(errorMessage(err)));
  }, []);

  async function select(providerId: string, modelId: string) {
    try {
      await setSelectedModel(providerId, modelId);
      toast.success("Model switched", { description: `${providerId} / ${modelId}` });
      await Promise.all([listModels().then(setModels), refreshStatus()]);
    } catch (err) {
      toast.error("Couldn't switch model", { description: errorMessage(err) });
    }
  }

  const current = status.selected;

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex-1 overflow-auto">
        <div className="flex items-baseline justify-between px-6 pt-5">
          <h2 className="m-0 text-[13px] font-semibold uppercase tracking-wide text-muted">Choose a model</h2>
          <span className="text-xs text-muted-2">
            Any provider Otto supports - configure a key in Settings to unlock its models.
          </span>
        </div>
        <div className="flex flex-col gap-6 px-6 pt-3 pb-7">
          {error ? (
            <div className="mx-auto my-16 text-center text-[13px] text-muted-2">Couldn't load models: {error}</div>
          ) : models === null || !current ? (
            <div className="mx-auto my-16 text-center text-[13px] text-muted-2">Loading models...</div>
          ) : (
            status.providers.map((provider) => {
              const configured =
                !provider.needs_api_key ||
                status.credentialStatus.find((s) => s.key === `${provider.id}-api-key`)?.configured;
              const providerModels = models.filter((m) => m.provider === provider.id);

              if (!configured) {
                return (
                  <section key={provider.id}>
                    <div className="mb-2 flex items-center justify-between">
                      <h3 className="text-sm font-bold text-muted-2">{provider.name}</h3>
                      <button
                        type="button"
                        onClick={onOpenSettings}
                        className="cursor-pointer border-none bg-transparent text-[12px] font-semibold text-accent-2"
                      >
                        Add API key in Settings &rarr;
                      </button>
                    </div>
                  </section>
                );
              }

              const customSelected = current.provider === provider.id && !providerModels.some((m) => m.id === current.model);
              const customValue = customValues[provider.id] ?? (customSelected ? current.model : "");

              return (
                <section key={provider.id}>
                  <div className="mb-2 flex items-center justify-between">
                    <h3 className="text-sm font-bold">{provider.name}</h3>
                  </div>
                  <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3.5">
                    {providerModels.map((m) => (
                      <ModelCard
                        key={m.id}
                        model={m}
                        selected={m.provider === current.provider && m.id === current.model}
                        onSelect={() => select(m.provider, m.id)}
                      />
                    ))}
                  </div>
                  <div className="mt-2.5 flex items-center gap-2">
                    <input
                      type="text"
                      value={customValue}
                      onChange={(e) => setCustomValues((prev) => ({ ...prev, [provider.id]: e.target.value }))}
                      placeholder={
                        provider.needs_api_key ? "Or type any other model id..." : "Model name pulled locally, e.g. llama3.1..."
                      }
                      className={cn(
                        "flex-1 rounded-lg border bg-surface-sunken px-2.5 py-1.5 text-[12.5px] font-mono focus:border-accent-2 focus:outline-none",
                        customSelected ? "border-accent-2" : "border-border",
                      )}
                    />
                    <button
                      type="button"
                      onClick={() => {
                        const value = customValue.trim();
                        if (value) select(provider.id, value);
                      }}
                      className="cursor-pointer rounded-lg border border-border bg-surface-sunken px-3 py-1.5 text-[12px] font-semibold hover:bg-surface"
                    >
                      Use
                    </button>
                  </div>
                </section>
              );
            })
          )}
        </div>
      </div>
    </section>
  );
}
