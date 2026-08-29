import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Sidebar } from "@/components/Sidebar";

function setup(activeTab: "dashboard" | "activities" | "models" | "skills" = "dashboard") {
  const onTabChange = vi.fn();
  const onSettingsClick = vi.fn();
  render(
    <Sidebar activeTab={activeTab} onTabChange={onTabChange} onSettingsClick={onSettingsClick} modelLabel="Anthropic · claude-opus-5" />,
  );
  return { onTabChange, onSettingsClick };
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
