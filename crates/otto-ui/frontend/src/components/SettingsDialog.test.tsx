import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SettingsDialog } from "@/components/SettingsDialog";
import { connectGoogleCalendar, setCredential } from "@/lib/tauri";
import type { CredentialStatus } from "@/lib/types";

vi.mock("@/lib/tauri", () => ({
  connectGoogleCalendar: vi.fn(),
  setCredential: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), info: vi.fn() }),
}));

const credentialStatus: CredentialStatus[] = [
  { key: "anthropic-api-key", label: "Anthropic", configured: true },
  { key: "github-token", label: "GitHub", configured: false },
  { key: "google-calendar-oauth", label: "Google Calendar", configured: false },
];

function setup(overrides: Partial<CredentialStatus[]> = credentialStatus) {
  const onOpenChange = vi.fn();
  const onSaved = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsDialog open onOpenChange={onOpenChange} credentialStatus={overrides as CredentialStatus[]} onSaved={onSaved} />,
  );
  return { onOpenChange, onSaved };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("SettingsDialog - rendering", () => {
  it("renders a paste field for normal credentials and a Connect button for the calendar OAuth row", () => {
    setup();
    expect(screen.getByLabelText("Anthropic")).toBeInTheDocument();
    expect(screen.getByLabelText("GitHub")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connect" })).toBeInTheDocument();
  });

  it("shows 'Reconnect' instead of 'Connect' once already configured", () => {
    setup([...credentialStatus.slice(0, 2), { ...credentialStatus[2], configured: true }]);
    expect(screen.getByRole("button", { name: "Reconnect" })).toBeInTheDocument();
  });

  it("shows configured/not set status next to each credential", () => {
    setup();
    expect(screen.getByText("configured")).toBeInTheDocument();
    expect(screen.getAllByText("not set")).toHaveLength(2);
  });
});

describe("SettingsDialog - saving credentials", () => {
  it("saves only the dirty fields, trimmed, then closes and refreshes", async () => {
    vi.mocked(setCredential).mockResolvedValue(undefined);
    const { onOpenChange, onSaved } = setup();

    await userEvent.type(screen.getByLabelText("GitHub"), "  ghp_abc123  ");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(setCredential).toHaveBeenCalledWith("github-token", "ghp_abc123"));
    expect(setCredential).toHaveBeenCalledOnce();
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(onSaved).toHaveBeenCalledOnce();
  });

  it("just closes without calling setCredential when nothing was typed", async () => {
    const { onOpenChange } = setup();
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(setCredential).not.toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("keeps the dialog open and shows an error toast if saving fails", async () => {
    vi.mocked(setCredential).mockRejectedValue("keychain unavailable");
    const { onOpenChange } = setup();

    await userEvent.type(screen.getByLabelText("GitHub"), "ghp_abc123");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(setCredential).toHaveBeenCalled());
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
  });
});

describe("SettingsDialog - Google Calendar connect", () => {
  it("runs the connect flow and refreshes status on success", async () => {
    vi.mocked(connectGoogleCalendar).mockResolvedValue(undefined);
    const { onSaved } = setup();

    await userEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => expect(connectGoogleCalendar).toHaveBeenCalledOnce());
    await waitFor(() => expect(onSaved).toHaveBeenCalledOnce());
  });

  it("does not refresh status if the connect flow fails", async () => {
    vi.mocked(connectGoogleCalendar).mockRejectedValue("no client id configured");
    const { onSaved } = setup();

    await userEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => expect(connectGoogleCalendar).toHaveBeenCalledOnce());
    expect(onSaved).not.toHaveBeenCalled();
  });
});
