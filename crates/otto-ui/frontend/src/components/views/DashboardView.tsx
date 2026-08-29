import { useEffect, useState } from "react";

import { CommandBar } from "@/components/CommandBar";
import { Icon } from "@/components/icons";
import { getGreetingName } from "@/lib/tauri";
import type { GithubPrStepData } from "@/lib/types";
import { cn } from "@/lib/utils";

function capitalize(s: string) {
  return s.length ? s[0].toUpperCase() + s.slice(1) : s;
}

function greetingFor(name: string | null) {
  const hour = new Date().getHours();
  const part = hour < 12 ? "morning" : hour < 18 ? "afternoon" : "evening";
  return `Good ${part}${name ? `, ${capitalize(name)}` : ""}`;
}

function PullRequestsCard({ data }: { data: GithubPrStepData | null }) {
  const prs = data?.pull_requests ?? [];
  return (
    <section className="flex flex-col gap-2.5 rounded-2xl border border-border bg-surface p-4 shadow-card">
      <div className="flex h-9 w-9 items-center justify-center rounded-[10px] bg-accent-2-soft text-accent-2">
        <Icon name="branch" className="h-5 w-5" />
      </div>
      <h3 className="m-0 text-sm font-bold">Pull Requests</h3>
      {data === null ? (
        <p className="m-0 flex-1 text-[12.5px] leading-relaxed text-muted">Ask &quot;show me my open PRs&quot; below.</p>
      ) : prs.length === 0 ? (
        <p className="m-0 flex-1 text-[12.5px] leading-relaxed text-muted">
          No open pull requests{data.repo ? ` in ${data.repo}` : ""}.
        </p>
      ) : (
        <>
          <div className="text-[12.5px] text-muted">
            {prs.length} open{data.repo ? ` in ${data.repo}` : ""}
          </div>
          {prs.slice(0, 4).map((pr) => (
            <div key={pr.number} className="mt-0.5 border-t border-border pt-2">
              <div className="text-[12.5px] font-semibold">
                {pr.draft && (
                  <span className="mr-1 rounded-full bg-accent-soft px-1.5 py-0.5 text-[10px] font-bold uppercase text-accent">
                    draft
                  </span>
                )}
                #{pr.number} {pr.title ?? ""}
              </div>
              <div className="mt-0.5 text-[11.5px] text-muted">
                {pr.head ?? ""} &rarr; {pr.base ?? ""} &middot; {pr.author ?? "unknown"}
              </div>
            </div>
          ))}
        </>
      )}
    </section>
  );
}

export function DashboardView({
  active,
  providerName,
}: {
  active: boolean;
  providerName: (providerId: string) => string;
}) {
  const [greetingName, setGreetingName] = useState<string | null>(null);
  const [prData, setPrData] = useState<GithubPrStepData | null>(null);

  useEffect(() => {
    getGreetingName()
      .then(setGreetingName)
      .catch((err) => console.warn("failed to load greeting name:", err));
  }, []);

  return (
    <section className={cn("flex min-h-0 flex-1 flex-col overflow-hidden", !active && "hidden")}>
      <div className="flex-1 overflow-auto">
        <div className="relative overflow-hidden border-b border-border bg-surface px-7 pt-8 pb-5">
          <svg
            className="pointer-events-none absolute -top-2 right-0 h-[140px] w-[340px] opacity-90"
            viewBox="0 0 340 140"
            fill="none"
          >
            <path d="M40 130 C 90 90, 120 40, 190 20" stroke="#2f6fed" strokeWidth={5} strokeLinecap="round" opacity={0.55} />
            <path d="M70 135 C 130 110, 160 60, 250 15" stroke="#c9821f" strokeWidth={5} strokeLinecap="round" opacity={0.5} />
            <path
              d="M110 138 C 170 125, 220 90, 320 45"
              stroke="#1f9d63"
              strokeWidth={5}
              strokeLinecap="round"
              opacity={0.45}
            />
          </svg>
          <h1 className="m-0 text-[26px] font-semibold tracking-tight">{greetingFor(greetingName)}</h1>
          <p className="mt-1.5 max-w-[520px] text-sm leading-relaxed text-muted">
            Ask Otto to check your PRs, review CI, or open one from a branch - or hold the mic and just say it.
          </p>
        </div>

        <div className="flex items-baseline justify-between px-6 pt-5">
          <h2 className="m-0 text-[13px] font-semibold uppercase tracking-wide text-muted">Overview</h2>
        </div>
        <main className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3.5 px-6 pt-3 pb-7">
          <PullRequestsCard data={prData} />
          <section className="flex flex-col gap-2.5 rounded-2xl border border-border bg-surface p-4 shadow-card">
            <div className="flex h-9 w-9 items-center justify-center rounded-[10px] bg-accent-soft text-accent">
              <Icon name="deploy" className="h-5 w-5" />
            </div>
            <h3 className="m-0 text-sm font-bold">Deployments</h3>
            <p className="m-0 flex-1 text-[12.5px] leading-relaxed text-muted">Deploy monitoring isn't wired up yet.</p>
          </section>
          <section className="flex flex-col gap-2.5 rounded-2xl border border-border bg-surface p-4 shadow-card">
            <div className="flex h-9 w-9 items-center justify-center rounded-[10px] bg-success-soft text-success">
              <Icon name="calendar" className="h-5 w-5" />
            </div>
            <h3 className="m-0 text-sm font-bold">Upcoming Meetings</h3>
            <p className="m-0 flex-1 text-[12.5px] leading-relaxed text-muted">Calendar integration isn't wired up yet.</p>
          </section>
        </main>
      </div>

      <CommandBar providerName={providerName} onPrData={setPrData} />
    </section>
  );
}
