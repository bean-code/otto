import { useEffect, useState } from "react";

import { listActivities } from "@/lib/tauri";
import type { ActivityEntry, ActivityKind } from "@/lib/types";
import { cn, errorMessage } from "@/lib/utils";

const KIND_LABEL: Record<ActivityKind, string> = {
  command: "Command",
  growth_digest: "Growth digest",
  schedule: "Schedule",
};

const KIND_DOT: Record<ActivityKind, string> = {
  command: "bg-accent-2",
  growth_digest: "bg-accent",
  schedule: "bg-muted-2",
};

function formatTimestamp(iso: string) {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function ActivitiesView({ providerName }: { providerName: (providerId: string) => string }) {
  const [entries, setEntries] = useState<ActivityEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listActivities(100)
      .then(setEntries)
      .catch((err) => setError(errorMessage(err)));
  }, []);

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex-1 overflow-auto">
        <div className="flex items-baseline justify-between px-6 pt-5">
          <h2 className="m-0 text-[13px] font-semibold uppercase tracking-wide text-muted">Activity feed</h2>
          {entries && entries.length > 0 && (
            <span className="text-xs text-muted-2">{entries.length} recent</span>
          )}
        </div>
        <main className="flex flex-col gap-2.5 px-6 pt-2 pb-7">
          {error ? (
            <div className="mx-auto my-16 text-center text-[13px] text-muted-2">Couldn't load activity: {error}</div>
          ) : entries === null ? (
            <div className="mx-auto my-16 text-center text-[13px] text-muted-2">Loading activity...</div>
          ) : entries.length === 0 ? (
            <div className="mx-auto my-16 text-center text-[13px] text-muted-2">
              Nothing yet - run a command from the Dashboard tab to get started.
            </div>
          ) : (
            entries.map((entry) => {
              const providerLabel = entry.provider ? `${providerName(entry.provider)} • ${entry.model}` : "";
              return (
                <div key={entry.id} className="flex gap-3 rounded-2xl border border-border bg-surface px-4 py-3 shadow-card">
                  <div className={cn("mt-1.5 h-2 w-2 flex-shrink-0 rounded-full", KIND_DOT[entry.kind] ?? "bg-muted-2")} />
                  <div className="min-w-0 flex-1">
                    <div className="text-[10px] font-bold uppercase tracking-wide text-muted-2">
                      {KIND_LABEL[entry.kind] ?? entry.kind}
                    </div>
                    <div className="flex items-baseline justify-between gap-2.5">
                      <div className="text-[13.5px] font-bold">{entry.title}</div>
                      <time className="whitespace-nowrap text-[11px] text-muted-2">{formatTimestamp(entry.timestamp)}</time>
                    </div>
                    <div className="mt-1 text-[13px] leading-relaxed text-muted">{entry.detail || "(no detail)"}</div>
                    {entry.steps.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-1.5">
                        {entry.steps.map((step, i) => (
                          <span
                            key={i}
                            className="rounded-full border border-[#d3e2fd] bg-accent-2-soft px-2 py-0.5 text-[11px] text-accent-2"
                          >
                            {step.tool_name}
                          </span>
                        ))}
                      </div>
                    )}
                    {providerLabel && <div className="mt-1.5 text-[11px] text-muted-2 opacity-70">via {providerLabel}</div>}
                  </div>
                </div>
              );
            })
          )}
        </main>
      </div>
    </section>
  );
}
