import { useEffect, useState } from "react";

import { Icon, type IconName } from "@/components/icons";
import { listSkills } from "@/lib/tauri";
import type { SkillInfo } from "@/lib/types";
import { errorMessage } from "@/lib/utils";

const SKILL_META: Record<string, { icon: IconName; tone: string; name: string }> = {
  github: { icon: "branch", tone: "bg-accent-2-soft text-accent-2", name: "GitHub" },
  deploy: { icon: "trendUp", tone: "bg-accent-soft text-accent", name: "Deploy" },
  calendar: { icon: "calendar", tone: "bg-success-soft text-success", name: "Calendar" },
  notify: { icon: "bell", tone: "bg-navy-soft text-navy", name: "Notify" },
};

export function SkillsView() {
  const [skills, setSkills] = useState<SkillInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listSkills()
      .then(setSkills)
      .catch((err) => setError(errorMessage(err)));
  }, []);

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex-1 overflow-auto">
        <div className="flex items-baseline justify-between px-6 pt-5">
          <h2 className="m-0 text-[13px] font-semibold uppercase tracking-wide text-muted">What Otto can do</h2>
        </div>
        <main className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3.5 px-6 pt-3 pb-7">
          {error ? (
            <div className="mx-auto my-16 text-center text-[13px] text-muted">Couldn't load skills: {error}</div>
          ) : skills === null ? (
            <div className="mx-auto my-16 text-center text-[13px] text-muted">Loading skills...</div>
          ) : (
            skills.map((skill) => {
              const meta = SKILL_META[skill.id] ?? { icon: "gear" as IconName, tone: "bg-accent-2-soft text-accent-2", name: skill.id };
              return (
                <section key={skill.id} className="flex flex-col gap-2.5 rounded-2xl border border-border bg-surface p-4 shadow-card">
                  <div className={`flex h-9 w-9 items-center justify-center rounded-[10px] ${meta.tone}`}>
                    <Icon name={meta.icon} className="h-5 w-5" />
                  </div>
                  <h3 className="m-0 text-sm font-bold">{meta.name}</h3>
                  <p className="m-0 text-[12.5px] leading-relaxed text-muted">
                    {skill.tools.length} tool{skill.tools.length === 1 ? "" : "s"} available to the agent loop.
                  </p>
                  <div className="flex flex-wrap gap-1.5">
                    {skill.tools.map((t) => (
                      <span key={t.name} className="rounded-full border border-border bg-surface-sunken px-1.5 py-0.5 text-[10.5px] text-muted">
                        {t.name}
                      </span>
                    ))}
                  </div>
                </section>
              );
            })
          )}
        </main>
        <div className="px-6 pt-1">
          <h2 className="m-0 text-[13px] font-semibold uppercase tracking-wide text-muted">Build your own</h2>
        </div>
        <div className="grid px-6 pt-3 pb-7">
          <section className="flex flex-col gap-2.5 rounded-2xl border border-border bg-surface p-4 shadow-card">
            <div className="flex h-9 w-9 items-center justify-center rounded-[10px] bg-navy-soft text-navy">
              <Icon name="code" className="h-5 w-5" />
            </div>
            <h3 className="m-0 text-sm font-bold">Write a new skill</h3>
            <p className="m-0 text-[12.5px] leading-relaxed text-muted">
              Every skill is a small Rust crate implementing the <code>Skill</code> trait - list your tools, handle the
              calls, register it. No plugin runtime, no sandboxing to fight: it's a normal crate in the workspace, like{" "}
              <code>otto-skill-notify</code>. See <strong>docs/SKILLS.md</strong> in the repo for a full walkthrough with a
              working example.
            </p>
          </section>
        </div>
      </div>
    </section>
  );
}
