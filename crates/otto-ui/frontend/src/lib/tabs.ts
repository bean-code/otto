import type { IconName } from "@/components/icons";

export type Tab = "dashboard" | "activities" | "models" | "skills";

export const TABS: { id: Tab; label: string; icon: IconName }[] = [
  { id: "dashboard", label: "Dashboard", icon: "grid" },
  { id: "activities", label: "Activities", icon: "clock" },
  { id: "models", label: "Models", icon: "sparkle" },
  { id: "skills", label: "Skills", icon: "code" },
];
