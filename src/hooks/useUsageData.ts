import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';
import { ProviderId, UsageSnapshot } from '@/types';
import { useUsageStore } from '@/stores/usageStore';
import { isTauriRuntime } from '@/utils/runtime';

const PROVIDERS: ProviderId[] = ['claude', 'codex', 'antigravity', 'cursor'];

// Bridges the backend event stream into the frontend store. On startup it tries
// to fetch fresh snapshots, then keeps the store in sync with `usage-updated`
// events emitted by the Rust backend.
export const useUsageData = () => {
  const fetchAll = useUsageStore((s) => s.fetchAll);
  const refreshAll = useUsageStore((s) => s.refreshAll);
  const fetchRecentActivity = useUsageStore((s) => s.fetchRecentActivity);
  const fetchAllAuthStates = useUsageStore((s) => s.fetchAllAuthStates);
  const upsertSnapshot = useUsageStore((s) => s.upsertSnapshot);

  useEffect(() => {
    const bootstrap = async () => {
      // Paint immediately from the last persisted snapshots, then let the
      // backend refresh in the background (its results arrive through the
      // `usage-updated` event). No waiting on network calls at startup.
      try {
        await fetchAll();
      } catch (error) {
        console.warn('[usage] could not load cached snapshots', error);
      }
      refreshAll().catch((error) => console.warn('[usage] background refresh failed', error));
      PROVIDERS.forEach((provider) => {
        fetchRecentActivity(provider, 120).catch(() => undefined);
      });
      fetchAllAuthStates().catch(() => undefined);
    };

    bootstrap().catch(() => undefined);

    if (!isTauriRuntime()) return;

    const unlistenPromise = listen<UsageSnapshot | UsageSnapshot[]>('usage-updated', (event) => {
      const payload = event.payload;
      if (Array.isArray(payload)) {
        // `refresh_all` emits a batch; merge each snapshot individually so the
        // store keeps its provider-keyed shape.
        payload.forEach((snapshot) => {
          upsertSnapshot(snapshot);
          fetchRecentActivity(snapshot.provider, 120).catch(() => undefined);
        });
        fetchAllAuthStates().catch(() => undefined);
        return;
      }
      upsertSnapshot(payload);
      fetchRecentActivity(payload.provider, 120).catch(() => undefined);
      fetchAllAuthStates().catch(() => undefined);
    });

    return () => {
      unlistenPromise.then((off) => off());
    };
  }, [fetchAll, fetchAllAuthStates, fetchRecentActivity, refreshAll, upsertSnapshot]);
};
