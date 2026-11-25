import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MarkId, ProviderMark } from '@/components/providers/ProviderLogo';
import { barColor } from '@/components/providers/OverviewCard';
import { ChevronLeft, ChevronRight } from '@/components/icons';
import { useLitellmStatus } from '@/hooks/useLitellmStatus';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUsageStore } from '@/stores/usageStore';
import { ProviderId, UsageSnapshot, UsageWindow } from '@/types';
import { ACTIVE_COLOR, getProviderAccessState } from '@/utils/providerAccess';
import { formatWindowPercent } from '@/utils/percent';
import { isTauriRuntime } from '@/utils/runtime';
import { displayWindows } from '@/utils/usageWindows';
import '@/components/capsule/capsule.css';

/*
 * Floating capsule ("isla"): one ink strip with a monogram per monitored AI.
 *
 * Honesty rules (DESIGN.md): it only renders what the backend already has —
 * cached snapshots plus `usage-updated` events — and never asks for a
 * refresh. A provider without a snapshot is not shown; one without a real
 * figure shows its status dot and no bar or percentage; estimates keep "~"
 * and Estimate Grey.
 *
 * Click: show / hide the main window. Drag (more than 4 px): move the capsule;
 * the backend saves the position. The chevron toggles inline percentages.
 */

const PROVIDERS: ProviderId[] = ['claude', 'codex', 'antigravity', 'cursor'];

const NAMES: Record<MarkId, string> = {
  claude: 'Claude',
  codex: 'ChatGPT',
  antigravity: 'Gemini',
  cursor: 'Cursor',
  litellm: 'LiteLLM',
};

const WINDOW_NAMES: Record<string, string> = {
  five_hour: 'ventana de 5 h',
  seven_day: 'ventana de 7 días',
  daily: 'cuota diaria',
  monthly: 'cuota mensual',
  weekly: 'cuota semanal',
  session: 'ventana de sesión',
};

type Item = {
  id: MarkId;
  color: string;
  hollow: boolean;
  window?: UsageWindow;
  title: string;
};

const itemFor = (provider: ProviderId, snapshot?: UsageSnapshot): Item | undefined => {
  if (!snapshot) return undefined;
  const access = getProviderAccessState(undefined, snapshot, undefined);
  const [primary] = displayWindows(snapshot);
  const name = NAMES[provider];
  const title = primary
    ? `${name} ${formatWindowPercent(primary)} · ${WINDOW_NAMES[primary.window_type] ?? 'ventana de uso'}${
        primary.accuracy === 'approximate' ? ' (estimación)' : ''
      } · ${access.label}`
    : `${name} · ${access.label}`;
  return {
    id: provider,
    color: access.color,
    hollow: access.color === ACTIVE_COLOR,
    window: primary,
    title,
  };
};

const Capsule = () => {
  const snapshots = useUsageStore((s) => s.snapshots);
  const fetchAll = useUsageStore((s) => s.fetchAll);
  const upsertSnapshot = useUsageStore((s) => s.upsertSnapshot);
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const litellm = useLitellmStatus();
  const [expanded, setExpanded] = useState(false);
  const pillRef = useRef<HTMLDivElement>(null);
  const press = useRef<{ x: number; y: number; dragging: boolean } | null>(null);

  // The page is a transparent window: no paper sheet behind the strip.
  useEffect(() => {
    document.documentElement.classList.add('qp-capsule-window');
    return () => document.documentElement.classList.remove('qp-capsule-window');
  }, []);

  // Read the stored snapshots once, then follow the backend's own refreshes.
  useEffect(() => {
    fetchAll().catch(() => undefined);
    if (!isTauriRuntime()) return;
    const off = listen<UsageSnapshot | UsageSnapshot[]>('usage-updated', (event) => {
      const payload = event.payload;
      (Array.isArray(payload) ? payload : [payload]).forEach(upsertSnapshot);
    });
    return () => {
      off.then((stop) => stop());
    };
  }, [fetchAll, upsertSnapshot]);

  // Provider toggles changed in the main window's Ajustes.
  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === 'quotapulse-settings') useSettingsStore.persist.rehydrate();
    };
    window.addEventListener('storage', onStorage);
    return () => window.removeEventListener('storage', onStorage);
  }, []);

  const items: Item[] = PROVIDERS.filter((id) => enabledProviders[id] !== false)
    .map((id) => itemFor(id, snapshots[id]))
    .filter((item): item is Item => !!item);
  if (litellm?.up) {
    items.push({ id: 'litellm', color: 'var(--qp-live)', hollow: false, title: 'LiteLLM · proxy local en línea' });
  }

  // Fit the window to the strip whenever its content changes size.
  useEffect(() => {
    const el = pillRef.current;
    if (!el || !isTauriRuntime()) return;
    const fit = () => {
      const r = el.getBoundingClientRect();
      invoke('capsule_resize', { width: Math.ceil(r.width) + 2, height: Math.ceil(r.height) + 2 }).catch(() => undefined);
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const toggleMain = () => {
    if (isTauriRuntime()) invoke('capsule_toggle_main').catch(() => undefined);
  };

  return (
    <div
      ref={pillRef}
      className={`qp-capsule ${expanded ? 'is-expanded' : ''}`}
      role="button"
      tabIndex={0}
      aria-label="QuotaPulse: mostrar u ocultar la ventana principal"
      title="Clic: mostrar u ocultar QuotaPulse · Arrastra para mover"
      onContextMenu={(e) => e.preventDefault()}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          toggleMain();
        }
      }}
      onPointerDown={(e) => {
        if (e.button === 0) press.current = { x: e.screenX, y: e.screenY, dragging: false };
      }}
      onPointerMove={(e) => {
        const p = press.current;
        if (!p || p.dragging) return;
        if (Math.hypot(e.screenX - p.x, e.screenY - p.y) > 4) {
          p.dragging = true;
          if (isTauriRuntime()) getCurrentWindow().startDragging().catch(() => undefined);
        }
      }}
      onPointerUp={() => {
        const p = press.current;
        press.current = null;
        if (p && !p.dragging) toggleMain();
      }}
    >
      {items.length === 0 ? (
        <span className="qp-capsule-empty">Sin datos todavía</span>
      ) : (
        items.map((item) => (
          <span key={item.id} className="qp-capsule-item" title={item.title}>
            <span className="qp-capsule-mark">
              <ProviderMark provider={item.id} size={16} />
              <span
                className={`qp-capsule-dot ${item.hollow ? 'is-hollow' : ''}`}
                style={{ background: item.hollow ? 'transparent' : item.color, borderColor: item.color }}
              />
            </span>
            {item.window && (
              <span className="qp-capsule-bar" aria-hidden="true">
                <span
                  style={{
                    width: `${Math.max(0, Math.min(100, item.window.utilization))}%`,
                    background: barColor(item.window),
                  }}
                />
              </span>
            )}
            {expanded && item.window && <span className="qp-capsule-pct">{formatWindowPercent(item.window)}</span>}
          </span>
        ))
      )}
      {items.some((item) => item.window) && (
        <button
          type="button"
          className="qp-capsule-toggle"
          aria-label={expanded ? 'Ocultar porcentajes' : 'Mostrar porcentajes'}
          title={expanded ? 'Ocultar porcentajes' : 'Mostrar porcentajes'}
          onPointerDown={(e) => e.stopPropagation()}
          onPointerUp={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
          onClick={(e) => {
            e.stopPropagation();
            setExpanded((v) => !v);
          }}
        >
          {expanded ? <ChevronLeft size={12} /> : <ChevronRight size={12} />}
        </button>
      )}
    </div>
  );
};

export default Capsule;
