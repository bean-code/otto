import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useOttoStatus } from "@/hooks/useOttoStatus";
import { getCredentialStatus, getSelectedModel, listProviders } from "@/lib/tauri";
import type { CredentialStatus, ProviderInfo, SelectedModel } from "@/lib/types";

vi.mock("@/lib/tauri", () => ({
  getCredentialStatus: vi.fn(),
  getSelectedModel: vi.fn(),
  listProviders: vi.fn(),
}));

const providers: ProviderInfo[] = [
  { id: "anthropic", name: "Anthropic", needs_api_key: true },
  { id: "ollama", name: "Ollama (local)", needs_api_key: false },
];
const credentialStatus: CredentialStatus[] = [{ key: "anthropic-api-key", label: "Anthropic", configured: true }];
const selected: SelectedModel = { provider: "anthropic", model: "claude-opus-5" };

beforeEach(() => {
  vi.mocked(getCredentialStatus).mockResolvedValue(credentialStatus);
  vi.mocked(getSelectedModel).mockResolvedValue(selected);
  vi.mocked(listProviders).mockResolvedValue(providers);
});

describe("useOttoStatus", () => {
  it("starts with empty status before refresh() is called", () => {
    const { result } = renderHook(() => useOttoStatus());
    expect(result.current.status).toEqual({ credentialStatus: [], providers: [], selected: null });
  });

  it("refresh() fetches all three in parallel and updates status", async () => {
    const { result } = renderHook(() => useOttoStatus());

    let returned: unknown;
    await act(async () => {
      returned = await result.current.refresh();
    });

    expect(result.current.status).toEqual({ credentialStatus, providers, selected });
    // The resolved value from refresh() itself should match, not just the
    // eventual state - callers (like App's onboarding-toast check) read it
    // directly rather than waiting on a re-render.
    expect(returned).toEqual({ credentialStatus, providers, selected });
  });

  it("providerName resolves a known id to its display name, and falls back to the raw id otherwise", async () => {
    const { result } = renderHook(() => useOttoStatus());
    await act(async () => {
      await result.current.refresh();
    });

    await waitFor(() => {
      expect(result.current.providerName("anthropic")).toBe("Anthropic");
    });
    expect(result.current.providerName("some-unknown-provider")).toBe("some-unknown-provider");
  });
});
