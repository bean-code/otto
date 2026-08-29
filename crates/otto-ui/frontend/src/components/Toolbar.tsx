import ottoMark from "@/assets/otto-mark.png";
import { Icon } from "@/components/icons";
import { StatusPill } from "@/components/StatusPill";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { TABS, type Tab } from "@/lib/tabs";

export function Toolbar({
  activeTab,
  onTabChange,
  onSettingsClick,
  modelPill,
  githubPill,
}: {
  activeTab: Tab;
  onTabChange: (tab: Tab) => void;
  onSettingsClick: () => void;
  modelPill: { ok: boolean; label: string };
  githubPill: { ok: boolean; label: string };
}) {
  return (
    <header className="drag-region flex select-none items-center gap-5 border-b border-border bg-surface px-5 py-3">
      <div className="flex items-center gap-2 text-[15px] font-bold">
        <img src={ottoMark} alt="" className="h-6 w-6 rounded-[7px]" />
        <span>Otto</span>
        <span className="ml-0.5 text-[11px] font-medium uppercase tracking-wide text-muted-2">autopilot</span>
      </div>

      <nav className="no-drag flex gap-0.5">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            onClick={() => onTabChange(tab.id)}
            className={cn(
              "cursor-pointer rounded-lg px-3.5 py-1.5 text-[13px] font-semibold transition-colors",
              activeTab === tab.id
                ? "bg-accent-2-soft text-ink"
                : "text-muted hover:bg-surface-sunken hover:text-ink",
            )}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      <div className="no-drag ml-auto flex items-center gap-2.5">
        <div className="flex gap-1.5">
          <StatusPill ok={modelPill.ok} label={modelPill.label} />
          <StatusPill ok={githubPill.ok} label={githubPill.label} />
        </div>
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="h-8 w-8 border-border-strong bg-surface text-ink hover:bg-surface-sunken"
          title="Settings"
          onClick={onSettingsClick}
        >
          <Icon name="settings" className="h-4.5 w-4.5" />
        </Button>
      </div>
    </header>
  );
}
