import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AuthGate } from "@/components/AuthGate";
import { signInWithPat, startGithubDeviceFlow } from "@/lib/tauri";
import type { Profile } from "@/lib/types";

let deviceCodeHandler: ((event: { payload: { user_code: string; verification_uri: string } }) => void) | undefined;

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_event: string, handler: typeof deviceCodeHandler) => {
    deviceCodeHandler = handler;
    return Promise.resolve(() => {});
  }),
}));

vi.mock("@/lib/tauri", () => ({
  startGithubDeviceFlow: vi.fn(),
  signInWithPat: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), info: vi.fn() }),
}));

const profile: Profile = {
  id: "octocat",
  display_name: "The Octocat",
  avatar_url: null,
  github_login: "octocat",
  created_at: "2026-01-01T00:00:00Z",
};

beforeEach(() => {
  vi.clearAllMocks();
  deviceCodeHandler = undefined;
});

describe("AuthGate - idle", () => {
  it("shows both sign-in paths", () => {
    render(<AuthGate onSignedIn={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Sign in with GitHub" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Paste a token instead" })).toBeInTheDocument();
  });
});

describe("AuthGate - device flow", () => {
  it("shows the code once the device-code event fires, and signs in once the flow resolves", async () => {
    let resolveFlow!: (profile: Profile) => void;
    vi.mocked(startGithubDeviceFlow).mockReturnValue(
      new Promise((resolve) => {
        resolveFlow = resolve;
      }),
    );
    const onSignedIn = vi.fn();
    render(<AuthGate onSignedIn={onSignedIn} />);

    await userEvent.click(screen.getByRole("button", { name: "Sign in with GitHub" }));
    expect(screen.getByText("Requesting a sign-in code...")).toBeInTheDocument();

    deviceCodeHandler?.({ payload: { user_code: "ABCD-1234", verification_uri: "https://github.com/login/device" } });
    await waitFor(() => expect(screen.getByText("ABCD-1234")).toBeInTheDocument());

    resolveFlow(profile);
    await waitFor(() => expect(onSignedIn).toHaveBeenCalledWith(profile));
  });

  it("shows an error toast and returns to idle if the device flow fails", async () => {
    const { toast } = await import("sonner");
    vi.mocked(startGithubDeviceFlow).mockRejectedValue("no client id configured");
    render(<AuthGate onSignedIn={vi.fn()} />);

    await userEvent.click(screen.getByRole("button", { name: "Sign in with GitHub" }));

    await waitFor(() => expect(toast.error).toHaveBeenCalledOnce());
    expect(screen.getByRole("button", { name: "Sign in with GitHub" })).toBeInTheDocument();
  });
});

describe("AuthGate - paste a token", () => {
  it("signs in with the trimmed token and calls onSignedIn", async () => {
    vi.mocked(signInWithPat).mockResolvedValue(profile);
    const onSignedIn = vi.fn();
    render(<AuthGate onSignedIn={onSignedIn} />);

    await userEvent.click(screen.getByRole("button", { name: "Paste a token instead" }));
    await userEvent.type(screen.getByLabelText("GitHub personal access token"), "  ghp_abc123  ");
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));

    await waitFor(() => expect(signInWithPat).toHaveBeenCalledWith("ghp_abc123"));
    await waitFor(() => expect(onSignedIn).toHaveBeenCalledWith(profile));
  });
});
