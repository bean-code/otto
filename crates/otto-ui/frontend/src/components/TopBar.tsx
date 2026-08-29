import { StatusPill } from "@/components/StatusPill";
import { TABS, type Tab } from "@/lib/tabs";

export function TopBar({
  activeTab,
  modelPill,
  githubPill,
}: {
  activeTab: Tab;
  modelPill: { ok: boolean; label: string };
  githubPill: { ok: boolean; label: string };
}) {
  const title = TABS.find((t) => t.id === activeTab)?.label ?? "Otto";

  return (
    <header className="drag-region flex select-none items-center border-b border-border bg-surface px-6 py-3.5">
      <h1 className="m-0 text-[15px] font-bold">{title}</h1>
      <div className="no-drag ml-auto flex gap-1.5">
        <StatusPill ok={modelPill.ok} label={modelPill.label} />
        <StatusPill ok={githubPill.ok} label={githubPill.label} />
      </div>
    </header>
  );
}
