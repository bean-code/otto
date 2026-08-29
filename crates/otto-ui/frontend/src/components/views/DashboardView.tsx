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

const SUGGESTIONS = ["Show my open PRs", "Check CI status", "Open a PR from a branch"];

function Orb() {
  return (
    <div className="relative flex h-20 w-20 items-center justify-center">
      <div
        className="absolute inset-0 rounded-full opacity-70 blur-xl"
        style={{ background: "radial-gradient(circle at 30% 30%, #eaf1ff, #2f6fed 45%, #996318 85%)" }}
      />
      <div
        className="relative h-16 w-16 animate-orb-breathe rounded-full"
        style={{
          background: "radial-gradient(circle at 35% 30%, #ffffff, #cfe0ff 30%, #7fa8f5 60%, #3a5fc9 100%)",
          boxShadow: "0 0 40px rgba(47,111,237,0.35), inset 0 0 16px rgba(255,255,255,0.6)",
        }}
      />
    </div>
  );
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

  const heroContent = (
    <div className="flex flex-col items-center gap-4 text-center">
      <Orb />
      <div>
        <h1 className="m-0 text-[26px] font-semibold tracking-tight">What can Otto do for you?</h1>
        <p className="mt-1.5 max-w-[440px] text-sm leading-relaxed text-muted">
          {greetingFor(greetingName)} - ask Otto to check your PRs, review CI, or open one from a branch, or hold the
          mic and just say it.
        </p>
      </div>
    </div>
  );

  return (
    <section className={cn("flex min-h-0 flex-1 flex-col overflow-hidden", !active && "hidden")}>
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        <CommandBar providerName={providerName} onPrData={setPrData} heroContent={heroContent} suggestions={SUGGESTIONS} />

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
    </section>
  );
}
