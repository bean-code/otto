import { useRef, useState } from "react";
import { toast } from "sonner";

import { Icon } from "@/components/icons";
import { runCommand, speak, startRecording, stopRecording } from "@/lib/tauri";
import type { AgentTurn, GithubPrStepData } from "@/lib/types";
import { cn, errorMessage } from "@/lib/utils";

interface TranscriptEntry {
  id: number;
  prompt: string;
  turn: AgentTurn | null;
  error: string | null;
}

let nextEntryId = 0;

export function CommandBar({
  providerName,
  onPrData,
}: {
  providerName: (providerId: string) => string;
  onPrData: (data: GithubPrStepData) => void;
}) {
  const [entries, setEntries] = useState<TranscriptEntry[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [recording, setRecording] = useState(false);
  const [talkBack, setTalkBack] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  async function submit(text: string) {
    const trimmed = text.trim();
    if (!trimmed || busy) return;

    setBusy(true);
    try {
      const turn = await runCommand(trimmed);
      setEntries((prev) => [...prev, { id: nextEntryId++, prompt: trimmed, turn, error: null }]);

      for (const step of turn.steps) {
        if (step.tool_name === "github_list_open_prs") {
          onPrData(step.data as GithubPrStepData);
        }
      }

      toast.success("Otto replied", {
        description: turn.reply ? (turn.reply.length > 120 ? `${turn.reply.slice(0, 117)}...` : turn.reply) : "Done.",
      });

      if (talkBack && turn.reply) {
        speak(turn.reply).catch((err) => console.warn("speak failed:", err));
      }
    } catch (err) {
      const message = errorMessage(err);
      setEntries((prev) => [...prev, { id: nextEntryId++, prompt: trimmed, turn: null, error: message }]);
      toast.error("Command failed", { description: message });
    } finally {
      setBusy(false);
      setInputValue("");
      inputRef.current?.focus();
    }
  }

  async function beginRecording() {
    if (recording) return;
    try {
      await startRecording();
      setRecording(true);
    } catch (err) {
      console.warn("voice input unavailable:", err);
    }
  }

  async function endRecording() {
    if (!recording) return;
    setRecording(false);
    try {
      const text = await stopRecording();
      if (text.trim()) {
        setInputValue(text.trim());
        await submit(text.trim());
      }
    } catch (err) {
      console.warn("transcription failed:", err);
    }
  }

  return (
    <footer className="flex flex-col gap-2.5 border-t border-border bg-surface px-5 pt-3 pb-4">
      <div className="flex max-h-56 flex-col-reverse gap-2 overflow-y-auto text-[13px]">
        {entries.map((entry) => (
          <div
            key={entry.id}
            className={cn(
              "animate-slide-in rounded-[10px] border border-border bg-surface-sunken px-2.5 py-2",
              entry.error && "text-danger",
            )}
          >
            <div className="mb-1 font-semibold text-muted before:mr-1 before:text-accent-2 before:content-['>_']">
              {entry.prompt}
            </div>
            <div className={cn("whitespace-pre-wrap leading-relaxed", entry.error && "text-danger")}>
              {entry.error ?? entry.turn?.reply ?? "(no reply text)"}
            </div>
            {!entry.error && entry.turn?.provider && (
              <div className="mt-1 text-[11px] text-muted opacity-70">
                via {providerName(entry.turn.provider)} • {entry.turn.model}
              </div>
            )}
            {entry.turn?.steps.map((step, i) => (
              <div key={i} className="mt-1 border-l-2 border-border-strong pl-2.5 text-xs text-muted">
                {step.tool_name} - {step.summary}
              </div>
            ))}
          </div>
        ))}
      </div>

      {busy && (
        <div className="flex items-center gap-2 text-xs text-muted">
          <span
            className="h-2.5 w-2.5 animate-spin-orb rounded-full"
            style={{ background: "conic-gradient(var(--color-accent), var(--color-accent-2), var(--color-accent))" }}
          />
          Otto is working on it...
        </div>
      )}

      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          submit(inputValue);
        }}
      >
        <button
          type="button"
          title="Hold to talk"
          disabled={busy}
          onMouseDown={beginRecording}
          onMouseUp={endRecording}
          onMouseLeave={endRecording}
          onTouchStart={(e) => {
            e.preventDefault();
            beginRecording();
          }}
          onTouchEnd={endRecording}
          className={cn(
            "flex w-11 cursor-pointer items-center justify-center rounded-[10px] border text-base transition-all",
            recording
              ? "animate-pulse-ring border-danger bg-danger text-white"
              : "border-border bg-surface-sunken text-ink",
          )}
        >
          <Icon name="mic" className="h-5 w-5" />
        </button>
        <input
          ref={inputRef}
          type="text"
          autoComplete="off"
          disabled={busy}
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          placeholder='Tell Otto what to do - e.g. "show me my open PRs"'
          className="flex-1 rounded-[10px] border border-border bg-surface-sunken px-3 py-2.5 text-sm text-ink transition-all focus:border-accent-2 focus:bg-surface focus:ring-3 focus:ring-accent-2-soft focus:outline-none disabled:opacity-60"
        />
        <button
          type="submit"
          disabled={busy}
          className="cursor-pointer rounded-[10px] border-none bg-navy px-5 text-sm font-bold text-white transition-all hover:brightness-125 active:scale-95 disabled:opacity-50"
        >
          Run
        </button>
      </form>
      <label className="flex items-center gap-1.5 text-xs text-muted">
        <input type="checkbox" checked={talkBack} onChange={(e) => setTalkBack(e.target.checked)} />
        Talk back (speak replies aloud)
      </label>
    </footer>
  );
}
