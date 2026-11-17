import { useCallback } from 'react';
import { useVisibleInterval } from '@/hooks/usePageVisible';
import { ProviderId } from '@/types';
import { useUsageStore } from '@/stores/usageStore';

const PROVIDERS: ProviderId[] = ['claude', 'codex', 'antigravity', 'cursor'];

// Provider health is polled separately from usage snapshots so the UI can
// distinguish "provider reachable" from "usage snapshot currently unavailable".
export const useProviderStatus = () => {
  const fetchStatus = useUsageStore((s) => s.fetchStatus);

  const run = useCallback(() => {
    PROVIDERS.forEach((provider) => {
      fetchStatus(provider).catch((error) => {
        console.warn(`[status] ${provider} status check failed`, error);
      });
    });
  }, [fetchStatus]);

  // Paused while the window is hidden in the tray; runs again on show.
  useVisibleInterval(run, 60_000);
};
