import ottoMark from "@/assets/otto-mark.png";
import { Icon } from "@/components/icons";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { TABS, type Tab } from "@/lib/tabs";
import type { Profile } from "@/lib/types";
import { cn } from "@/lib/utils";

function NavItem({
  active,
  icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: Parameters<typeof Icon>[0]["name"];
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex w-full cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13px] font-semibold transition-colors",
        active ? "bg-accent-2-soft text-ink" : "text-muted hover:bg-surface-sunken hover:text-ink",
      )}
    >
      <Icon name={icon} className="h-4.5 w-4.5 flex-shrink-0" />
      <span className="truncate">{label}</span>
    </button>
  );
}

export function Sidebar({
  activeTab,
  onTabChange,
  onSettingsClick,
  modelLabel,
  currentProfile = null,
  profiles = [],
  onSwitchProfile,
  onSignInAnother,
  onSignOut,
}: {
  activeTab: Tab;
  onTabChange: (tab: Tab) => void;
  onSettingsClick: () => void;
  modelLabel: string;
  currentProfile?: Profile | null;
  profiles?: Profile[];
  onSwitchProfile?: (profileId: string) => void;
  onSignInAnother?: () => void;
  onSignOut?: () => void;
}) {
  const dashboardTab = TABS[0];
  const workspaceTabs = TABS.slice(1);

  return (
    <aside className="drag-region flex w-60 flex-shrink-0 select-none flex-col border-r border-border bg-surface">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className="no-drag flex w-full cursor-pointer items-center gap-2 px-4 py-4 text-left text-[15px] font-bold hover:bg-surface-sunken"
          >
            {currentProfile?.avatar_url ? (
              <img src={currentProfile.avatar_url} alt="" className="h-6 w-6 flex-shrink-0 rounded-full" />
            ) : (
              <img src={ottoMark} alt="" className="h-6 w-6 flex-shrink-0 rounded-[7px]" />
            )}
            <span className="flex-1 truncate">{currentProfile?.display_name ?? "Otto"}</span>
            <Icon name="chevronDown" className="h-3.5 w-3.5 flex-shrink-0 text-muted" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start">
          {profiles.length > 0 && (
            <>
              <DropdownMenuLabel>Profiles</DropdownMenuLabel>
              {profiles.map((profile) => (
                <DropdownMenuItem key={profile.id} onSelect={() => onSwitchProfile?.(profile.id)}>
                  <span className="flex-1 truncate">{profile.display_name}</span>
                  {currentProfile?.id === profile.id && <span className="text-xs text-muted">Active</span>}
                </DropdownMenuItem>
              ))}
              <DropdownMenuSeparator />
            </>
          )}
          <DropdownMenuItem onSelect={() => onSignInAnother?.()}>Sign in another account</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => onSignOut?.()}>Sign out</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <nav className="no-drag flex flex-col gap-1 px-3">
        <NavItem
          active={activeTab === dashboardTab.id}
          icon={dashboardTab.icon}
          label={dashboardTab.label}
          onClick={() => onTabChange(dashboardTab.id)}
        />

        <button
          type="button"
          onClick={() => onTabChange("models")}
          className="mt-1 flex items-center gap-2 rounded-lg border border-border px-2.5 py-2 text-left text-[13px] font-medium text-ink transition-colors hover:bg-surface-sunken"
          title="Switch model"
        >
          <Icon name="sparkle" className="h-4 w-4 flex-shrink-0 text-accent-2" />
          <span className="flex-1 truncate">{modelLabel}</span>
          <Icon name="chevronDown" className="h-3.5 w-3.5 flex-shrink-0 text-muted" />
        </button>

        <div className="mt-4 mb-1 px-2.5 text-[10px] font-bold tracking-wide text-muted uppercase">Workspace</div>
        {workspaceTabs.map((tab) => (
          <NavItem
            key={tab.id}
            active={activeTab === tab.id}
            icon={tab.icon}
            label={tab.label}
            onClick={() => onTabChange(tab.id)}
          />
        ))}
      </nav>

      <div className="flex-1" />

      <div className="no-drag border-t border-border px-3 py-3">
        <NavItem active={false} icon="settings" label="Settings" onClick={onSettingsClick} />
      </div>
    </aside>
  );
}
