import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { CommandBar } from "@/components/CommandBar";
import { runCommand, speak, startRecording, stopRecording } from "@/lib/tauri";
import type { AgentTurn } from "@/lib/types";

vi.mock("@/lib/tauri", () => ({
  runCommand: vi.fn(),
  speak: vi.fn(),
  startRecording: vi.fn(),
  stopRecording: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), info: vi.fn() }),
}));

function turn(overrides: Partial<AgentTurn> = {}): AgentTurn {
  return { reply: "Done.", steps: [], provider: "anthropic", model: "claude-opus-5", ...overrides };
}

const providerName = (id: string) => id;
const onPrData = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
});

describe("CommandBar - hero mode (empty transcript)", () => {
  it("renders heroContent and suggestion pills, and hides the docked talk-back toggle", () => {
    render(
      <CommandBar
        providerName={providerName}
        onPrData={onPrData}
        heroContent={<h1>What can Otto do for you?</h1>}
        suggestions={["Show my open PRs"]}
      />,
    );

    expect(screen.getByText("What can Otto do for you?")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show my open PRs" })).toBeInTheDocument();
    expect(screen.queryByText("Talk back (speak replies aloud)")).not.toBeInTheDocument();
  });

  it("clicking a suggestion pill submits it directly", async () => {
    vi.mocked(runCommand).mockResolvedValue(turn({ reply: "2 open PRs" }));
    render(
      <CommandBar providerName={providerName} onPrData={onPrData} suggestions={["Show my open PRs"]} />,
    );

    await userEvent.click(screen.getByRole("button", { name: "Show my open PRs" }));

    await waitFor(() => expect(runCommand).toHaveBeenCalledWith("Show my open PRs"));
  });

  it("submits the typed input and transitions to the docked transcript view on success", async () => {
    vi.mocked(runCommand).mockResolvedValue(turn({ reply: "2 open PRs" }));
    render(<CommandBar providerName={providerName} onPrData={onPrData} />);

    const input = screen.getByPlaceholderText(/Build me an agent/i);
    await userEvent.type(input, "show me my open PRs");
    await userEvent.click(screen.getByTitle("Send"));

    expect(runCommand).toHaveBeenCalledWith("show me my open PRs");
    await waitFor(() => expect(screen.getByText("2 open PRs")).toBeInTheDocument());
    // Docked view renders the talk-back toggle; hero view doesn't.
    expect(screen.getByText("Talk back (speak replies aloud)")).toBeInTheDocument();
  });

  it("records a failed command as an error entry and still leaves the hero", async () => {
    vi.mocked(runCommand).mockRejectedValue("no GitHub token configured");
    render(<CommandBar providerName={providerName} onPrData={onPrData} />);

    const input = screen.getByPlaceholderText(/Build me an agent/i);
    await userEvent.type(input, "show me my open PRs{Enter}");

    await waitFor(() => expect(screen.getByText("no GitHub token configured")).toBeInTheDocument());
  });
});

describe("CommandBar - docked mode (active transcript)", () => {
  async function renderWithOneEntry() {
    vi.mocked(runCommand).mockResolvedValue(turn({ reply: "first reply" }));
    render(<CommandBar providerName={providerName} onPrData={onPrData} />);
    const input = screen.getByPlaceholderText(/Build me an agent/i);
    await userEvent.type(input, "first command{Enter}");
    await waitFor(() => expect(screen.getByText("first reply")).toBeInTheDocument());
  }

  it("shows the provider/model attribution line for a successful reply", async () => {
    await renderWithOneEntry();
    expect(screen.getByText(/via anthropic • claude-opus-5/)).toBeInTheDocument();
  });

  it("speaks the reply when talk-back is checked", async () => {
    vi.mocked(speak).mockResolvedValue(undefined);
    await renderWithOneEntry();

    await userEvent.click(screen.getByLabelText("Talk back (speak replies aloud)"));
    vi.mocked(runCommand).mockResolvedValue(turn({ reply: "second reply" }));

    const input = screen.getByPlaceholderText(/Tell Otto what to do/i);
    await userEvent.type(input, "second command{Enter}");

    await waitFor(() => expect(speak).toHaveBeenCalledWith("second reply"));
  });

  it("does not speak when talk-back is left unchecked", async () => {
    await renderWithOneEntry();
    expect(speak).not.toHaveBeenCalled();
  });
});

describe("CommandBar - push-to-talk", () => {
  it("starts recording on mousedown and stops+submits on mouseup", async () => {
    vi.mocked(startRecording).mockResolvedValue(undefined);
    vi.mocked(stopRecording).mockResolvedValue("show me my open PRs");
    vi.mocked(runCommand).mockResolvedValue(turn({ reply: "2 open PRs" }));

    render(<CommandBar providerName={providerName} onPrData={onPrData} />);
    const micButton = screen.getByTitle("Hold to talk");

    fireEvent.mouseDown(micButton);
    await waitFor(() => expect(startRecording).toHaveBeenCalled());

    fireEvent.mouseUp(micButton);
    await waitFor(() => expect(stopRecording).toHaveBeenCalled());
    await waitFor(() => expect(runCommand).toHaveBeenCalledWith("show me my open PRs"));
  });

  it("does not submit anything when transcription comes back empty", async () => {
    vi.mocked(startRecording).mockResolvedValue(undefined);
    vi.mocked(stopRecording).mockResolvedValue("");

    render(<CommandBar providerName={providerName} onPrData={onPrData} />);
    const micButton = screen.getByTitle("Hold to talk");

    fireEvent.mouseDown(micButton);
    await waitFor(() => expect(startRecording).toHaveBeenCalled());
    fireEvent.mouseUp(micButton);

    await waitFor(() => expect(stopRecording).toHaveBeenCalled());
    expect(runCommand).not.toHaveBeenCalled();
  });
});
