import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Sidebar } from "@/components/Sidebar";
import type { Profile } from "@/lib/types";

const profile: Profile = {
  id: "octocat",
  display_name: "The Octocat",
  avatar_url: null,
  github_login: "octocat",
  created_at: "2026-01-01T00:00:00Z",
};

function setup(
  activeTab: "dashboard" | "activities" | "models" | "skills" = "dashboard",
  overrides: Partial<{ currentProfile: Profile | null; profiles: Profile[] }> = {},
) {
  const onTabChange = vi.fn();
  const onSettingsClick = vi.fn();
  const onSwitchProfile = vi.fn();
  const onSignInAnother = vi.fn();
  const onSignOut = vi.fn();
  render(
    <Sidebar
      activeTab={activeTab}
      onTabChange={onTabChange}
      onSettingsClick={onSettingsClick}
      modelLabel="Anthropic · claude-opus-5"
      currentProfile={overrides.currentProfile}
      profiles={overrides.profiles}
      onSwitchProfile={onSwitchProfile}
      onSignInAnother={onSignInAnother}
      onSignOut={onSignOut}
    />,
  );
  return { onTabChange, onSettingsClick, onSwitchProfile, onSignInAnother, onSignOut };
}

describe("Sidebar", () => {
  it("renders the brand, every tab, the model-switcher label, and Settings", () => {
    setup();
    expect(screen.getByText("Otto")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Dashboard/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Activities/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Models/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Skills/ })).toBeInTheDocument();
    expect(screen.getByText("Anthropic · claude-opus-5")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Settings/ })).toBeInTheDocument();
  });

  it("marks only the active tab with the active-nav styling", () => {
    setup("activities");
    const dashboard = screen.getByRole("button", { name: /Dashboard/ });
    const activities = screen.getByRole("button", { name: /Activities/ });

    expect(activities.className).toContain("bg-accent-2-soft");
    expect(dashboard.className).not.toContain("bg-accent-2-soft");
  });

  it("clicking a workspace tab reports that tab's id", async () => {
    const { onTabChange } = setup();
    await userEvent.click(screen.getByRole("button", { name: /Models/ }));
    expect(onTabChange).toHaveBeenCalledWith("models");
  });

  it("clicking Dashboard reports dashboard even when already active elsewhere", async () => {
    const { onTabChange } = setup("skills");
    await userEvent.click(screen.getByRole("button", { name: /Dashboard/ }));
    expect(onTabChange).toHaveBeenCalledWith("dashboard");
  });

  it("clicking the model-switcher pill navigates to the Models tab", async () => {
    const { onTabChange } = setup();
    await userEvent.click(screen.getByTitle("Switch model"));
    expect(onTabChange).toHaveBeenCalledWith("models");
  });

  it("clicking Settings calls onSettingsClick, not onTabChange", async () => {
    const { onTabChange, onSettingsClick } = setup();
    await userEvent.click(screen.getByRole("button", { name: /Settings/ }));
    expect(onSettingsClick).toHaveBeenCalledOnce();
    expect(onTabChange).not.toHaveBeenCalled();
  });
});

describe("Sidebar - profile menu", () => {
  it("shows 'Otto' when nobody is signed in", () => {
    setup();
    expect(screen.getByRole("button", { name: "Otto" })).toBeInTheDocument();
  });

  it("shows the signed-in profile's display name", () => {
    setup("dashboard", { currentProfile: profile, profiles: [profile] });
    expect(screen.getByRole("button", { name: /The Octocat/ })).toBeInTheDocument();
  });

  it("lists known profiles and switching one calls onSwitchProfile", async () => {
    const other: Profile = { ...profile, id: "other", display_name: "Other Account" };
    const { onSwitchProfile } = setup("dashboard", { currentProfile: profile, profiles: [profile, other] });

    await userEvent.click(screen.getByRole("button", { name: /The Octocat/ }));
    await userEvent.click(screen.getByRole("menuitem", { name: /Other Account/ }));

    expect(onSwitchProfile).toHaveBeenCalledWith("other");
  });

  it("'Sign in another account' calls onSignInAnother", async () => {
    const { onSignInAnother } = setup("dashboard", { currentProfile: profile, profiles: [profile] });

    await userEvent.click(screen.getByRole("button", { name: /The Octocat/ }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Sign in another account" }));

    expect(onSignInAnother).toHaveBeenCalledOnce();
  });

  it("'Sign out' calls onSignOut", async () => {
    const { onSignOut } = setup("dashboard", { currentProfile: profile, profiles: [profile] });

    await userEvent.click(screen.getByRole("button", { name: /The Octocat/ }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Sign out" }));

    expect(onSignOut).toHaveBeenCalledOnce();
  });
});
