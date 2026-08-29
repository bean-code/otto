export type Tab = "dashboard" | "activities" | "models" | "skills";

export const TABS: { id: Tab; label: string }[] = [
  { id: "dashboard", label: "Dashboard" },
  { id: "activities", label: "Activities" },
  { id: "models", label: "Models" },
  { id: "skills", label: "Skills" },
];
