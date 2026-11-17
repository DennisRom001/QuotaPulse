import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize } from '@tauri-apps/api/dpi';
import { isTauriRuntime } from '@/utils/runtime';

/** Compact widget: fixed size, like a desktop gadget. */
const WIDGET_SIZE = { width: 360, height: 274 };
/** Dashboard: freely resizable above a comfortable minimum, no maximum. */
const DASHBOARD_MIN = { width: 360, height: 420 };
const DASHBOARD_DEFAULT = { width: 420, height: 640 };
const SIZE_KEY = 'qp.dashboardSize';

const readSavedSize = () => {
  try {
    const raw = localStorage.getItem(SIZE_KEY);
    const parsed = raw ? (JSON.parse(raw) as { width: number; height: number }) : null;
    if (parsed && parsed.width >= DASHBOARD_MIN.width && parsed.height >= DASHBOARD_MIN.height) return parsed;
  } catch {
    /* storage unavailable: use the default */
  }
  return DASHBOARD_DEFAULT;
};

const saveSize = (size: { width: number; height: number }) => {
  try {
    localStorage.setItem(SIZE_KEY, JSON.stringify(size));
  } catch {
    /* not critical */
  }
};

/**
 * Sizes the Tauri window for widget vs. dashboard mode.
 *
 * Widget mode is a fixed compact gadget. Dashboard mode is resizable with a
 * minimum size and no maximum; its last size is remembered across launches.
 */
export const useWidgetResize = (widgetMode: boolean) => {
  useEffect(() => {
    if (!isTauriRuntime()) return;
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    let saveTimer: number | undefined;
    let cancelled = false;

    (async () => {
      try {
        await win.setResizable(true);
        // Relax constraints first so the new size can always apply.
        await win.setSizeConstraints({ minWidth: 200, minHeight: 100 });
        if (widgetMode) {
          if (await win.isMaximized()) await win.unmaximize();
          await win.setSize(new LogicalSize(WIDGET_SIZE.width, WIDGET_SIZE.height));
          await win.setSizeConstraints({
            minWidth: WIDGET_SIZE.width,
            maxWidth: WIDGET_SIZE.width,
            minHeight: WIDGET_SIZE.height,
            maxHeight: WIDGET_SIZE.height,
          });
          await win.setResizable(false);
          return;
        }
        const size = readSavedSize();
        await win.setSize(new LogicalSize(size.width, size.height));
        await win.setSizeConstraints({ minWidth: DASHBOARD_MIN.width, minHeight: DASHBOARD_MIN.height });
        if (cancelled) return;
        // Remember the size the user drags to (debounced; skipped while maximized).
        unlisten = await win.onResized(() => {
          window.clearTimeout(saveTimer);
          saveTimer = window.setTimeout(async () => {
            try {
              if (await win.isMaximized()) return;
              const factor = await win.scaleFactor();
              const logical = (await win.innerSize()).toLogical(factor);
              saveSize({ width: Math.round(logical.width), height: Math.round(logical.height) });
            } catch {
              /* window closing */
            }
          }, 400);
        });
      } catch (err) {
        console.error('resize failed', err);
      }
    })();

    return () => {
      cancelled = true;
      window.clearTimeout(saveTimer);
      unlisten?.();
    };
  }, [widgetMode]);
};
