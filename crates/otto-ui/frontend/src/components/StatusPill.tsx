import { cn } from "@/lib/utils";

export function StatusPill({ ok, label }: { ok: boolean; label: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[11px] font-semibold tracking-wide",
        ok ? "border-success/30 bg-success-soft text-success" : "border-accent/30 bg-accent-soft text-accent",
      )}
    >
      <span className={cn("h-1.5 w-1.5 rounded-full", ok ? "bg-success" : "bg-accent")} />
      <span>{label}</span>
    </span>
  );
}
