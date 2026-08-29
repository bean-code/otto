import { useRef, useState } from "react";
import type { ReactNode } from "react";
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
  heroContent,
  suggestions,
}: {
  providerName: (providerId: string) => string;
  onPrData: (data: GithubPrStepData) => void;
  /** Rendered above the input, only while the transcript is still empty. */
  heroContent?: ReactNode;
  /** Quick-fill pills shown under the input, only while the transcript is still empty. */
  suggestions?: string[];
}) {
  const [entries, setEntries] = useState<TranscriptEntry[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [recording, setRecording] = useState(false);
  const [talkBack, setTalkBack] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const isEmpty = entries.length === 0;

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
        speak(turn.reply).catch((err) => toast.error("Talk-back failed", { description: errorMessage(err) }));
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
      toast.error("Voice input unavailable", { description: errorMessage(err) });
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
      } else {
        toast.info("Didn't catch that", { description: "No speech was detected in the recording." });
      }
    } catch (err) {
      toast.error("Transcription failed", { description: errorMessage(err) });
    }
  }

  const micButton = (
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
        "flex h-10 w-10 flex-shrink-0 cursor-pointer items-center justify-center rounded-full border transition-all",
        recording ? "animate-pulse-ring border-danger bg-danger text-white" : "border-border bg-surface text-ink",
      )}
    >
      <Icon name="mic" className="h-4.5 w-4.5" />
    </button>
  );

  const thinkingIndicator = busy && (
    <div className="flex items-center gap-2 text-xs text-muted">
      <span
        className="h-2.5 w-2.5 animate-spin-orb rounded-full"
        style={{ background: "conic-gradient(var(--color-accent), var(--color-accent-2), var(--color-accent))" }}
      />
      Otto is working on it...
    </div>
  );

  if (isEmpty) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-6 px-6 pb-10">
        {heroContent}

        <form
          className="w-full max-w-[680px] rounded-2xl border border-border bg-surface shadow-elevated"
          onSubmit={(e) => {
            e.preventDefault();
            submit(inputValue);
          }}
        >
          <div className="flex items-start gap-2.5 px-4 pt-4">
            <Icon name="sparkle" className="mt-0.5 h-4 w-4 flex-shrink-0 text-accent-2" />
            <input
              ref={inputRef}
              type="text"
              autoComplete="off"
              disabled={busy}
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              placeholder='Build me an agent that... - or ask "show me my open PRs"'
              className="flex-1 border-none bg-transparent text-sm text-ink outline-none placeholder:text-muted disabled:opacity-60"
            />
          </div>
          <div className="flex items-center justify-end gap-2 px-4 pt-6 pb-3.5">
            {micButton}
            <button
              type="submit"
              disabled={busy}
              title="Send"
              className="flex h-10 w-10 flex-shrink-0 cursor-pointer items-center justify-center rounded-full border-none bg-navy text-white transition-all hover:brightness-125 active:scale-95 disabled:opacity-50"
            >
              <Icon name="send" className="h-4.5 w-4.5" />
            </button>
          </div>
        </form>

        {thinkingIndicator}

        {suggestions && suggestions.length > 0 && (
          <div className="flex flex-wrap justify-center gap-2">
            {suggestions.map((s) => (
              <button
                key={s}
                type="button"
                disabled={busy}
                onClick={() => submit(s)}
                className="cursor-pointer rounded-full border border-border bg-surface-sunken px-3.5 py-1.5 text-[12.5px] font-medium text-ink transition-colors hover:bg-surface disabled:opacity-50"
              >
                {s}
              </button>
            ))}
          </div>
        )}
      </div>
    );
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

      {thinkingIndicator}

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
