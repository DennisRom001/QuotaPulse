import { useEffect, useSyncExternalStore } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime } from '@/utils/runtime';

/**
 * Whether the UI is on screen. False while the window is hidden in the tray
 * or minimised, so timers and polls that only feed the UI can pause and the
 * app idles at minimum CPU. The backend keeps polling for the tray icons.
 *
 * Sources: the page's `visibilitychange` (WebView2 reports minimise/hide),
 * plus Tauri focus/blur events checked against `isVisible`/`isMinimized`,
 * since hiding to the tray does not always reach the page.
 */
let visible = typeof document === 'undefined' || document.visibilityState === 'visible';
const listeners = new Set<() => void>();
const set = (next: boolean) => {
  if (next === visible) return;
  visible = next;
  listeners.forEach((l) => l());
};

let wired = false;
const wire = () => {
  if (wired || typeof document === 'undefined') return;
  wired = true;
  document.addEventListener('visibilitychange', () => set(document.visibilityState === 'visible'));
  if (!isTauriRuntime()) return;
  const win = getCurrentWindow();
  const check = async () => {
    try {
      set((await win.isVisible()) && !(await win.isMinimized()) && document.visibilityState === 'visible');
    } catch {
      /* window closing */
    }
  };
  void win.onFocusChanged(() => void check());
  void win.onResized(() => void check());
};

const subscribe = (listener: () => void) => {
  wire();
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const usePageVisible = () => useSyncExternalStore(subscribe, () => visible, () => true);

/**
 * `setInterval` that runs only while the UI is visible, and fires once right
 * away when the window comes back so the screen is current.
 */
export const useVisibleInterval = (callback: () => void, ms: number, runOnShow = true) => {
  const isVisible = usePageVisible();
  useEffect(() => {
    if (!isVisible) return;
    if (runOnShow) callback();
    const id = window.setInterval(callback, ms);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isVisible, ms]);
};
