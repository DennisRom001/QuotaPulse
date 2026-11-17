import { useEffect, useState } from 'react';
import { usePageVisible } from '@/hooks/usePageVisible';
import { invoke } from '@tauri-apps/api/core';
import { ProviderId } from '@/types';
import { isTauriRuntime } from '@/utils/runtime';

export type LitellmStatus = { up: boolean; base_url?: string | null; providers: ProviderId[] };

const POLL_MS = 30_000;

/** Local LiteLLM Proxy state, refreshed every 30s (the backend caches probes). */
export const useLitellmStatus = (): LitellmStatus | undefined => {
  const [status, setStatus] = useState<LitellmStatus>();
  const visible = usePageVisible();
  useEffect(() => {
    // Only polled while the window is on screen.
    if (!isTauriRuntime() || !visible) return;
    let alive = true;
    const load = () =>
      invoke<LitellmStatus>('get_litellm_status')
        .then((s) => alive && setStatus(s))
        .catch(() => alive && setStatus({ up: false, providers: [] }));
    load();
    const timer = window.setInterval(load, POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [visible]);
  return status;
};
