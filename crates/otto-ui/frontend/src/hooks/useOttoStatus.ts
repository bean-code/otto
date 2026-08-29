import { useCallback, useState } from "react";

import { getCredentialStatus, getSelectedModel, listProviders } from "@/lib/tauri";
import type { CredentialStatus, ProviderInfo, SelectedModel } from "@/lib/types";

export interface OttoStatus {
  credentialStatus: CredentialStatus[];
  providers: ProviderInfo[];
  selected: SelectedModel | null;
}

/**
 * Holds the credential-status / provider-catalog / selected-model snapshot
 * shared across the toolbar pills, the transcript's "via <provider>" tags,
 * the Activities feed, and the Settings dialog - mirrors the original
 * `credentialStatusCache`/`providersCache` module-scope variables plus a
 * `refreshStatus()` equivalent, just lifted into a hook so every consumer
 * re-renders when it changes instead of reading stale globals.
 */
export function useOttoStatus() {
  const [status, setStatus] = useState<OttoStatus>({ credentialStatus: [], providers: [], selected: null });

  const refresh = useCallback(async () => {
    const [credentialStatus, selected, providers] = await Promise.all([
      getCredentialStatus(),
      getSelectedModel(),
      listProviders(),
    ]);
    const next = { credentialStatus, providers, selected };
    setStatus(next);
    return next;
  }, []);

  const providerName = useCallback(
    (providerId: string) => status.providers.find((p) => p.id === providerId)?.name ?? providerId,
    [status.providers],
  );

  return { status, refresh, providerName };
}
