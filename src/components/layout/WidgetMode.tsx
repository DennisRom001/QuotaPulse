import { useEffect, useState } from 'react';
import { Gauge, History, Refresh } from '@/components/icons';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import WidgetActivityView from '@/components/layout/WidgetActivityView';
import InkBlock from '@/components/providers/InkBlock';
import { ProviderMark } from '@/components/providers/ProviderLogo';
import { barColor } from '@/components/providers/OverviewCard';
import { useUsageStore } from '@/stores/usageStore';
import { ModelBreakdownEntry, ProviderId, ProviderStatus, RecentActivityEntry, UsageSnapshot, WindowType } from '@/types';
import { getProviderAccessState } from '@/utils/providerAccess';
import { isTauriRuntime } from '@/utils/runtime';
import { displayWindows } from '@/utils/usageWindows';
import { formatWindowPercent } from '@/utils/percent';

const meta: Record<ProviderId, { name: string; tint: 'claude' | 'codex' | 'antigravity' | 'cursor' }> = {
  claude: { name: 'Claude', tint: 'claude' },
  codex: { name: 'ChatGPT', tint: 'codex' },
  antigravity: { name: 'Gemini', tint: 'antigravity' },
  cursor: { name: 'Cursor', tint: 'cursor' },
};

const providers: ProviderId[] = ['claude', 'codex', 'antigravity', 'cursor'];

const widgetWindowLabel = (windowType?: WindowType) => {
  switch (windowType) {
    case 'five_hour':
      return '5H';
    case 'seven_day':
      return '7D';
    case 'daily':
      return 'DÍA';
    case 'monthly':
      return 'MES';
    case 'weekly':
      return 'SEM';
    case 'session':
      return 'SES';
    default:
      return 'VEN';
  }
};

type WidgetModeProps = {
  snapshots: Record<ProviderId, UsageSnapshot | undefined>;
  statuses: Partial<Record<ProviderId, ProviderStatus>>;
  modelBreakdowns: Record<ProviderId, ModelBreakdownEntry[]>;
  recentActivity: Record<ProviderId, RecentActivityEntry[]>;
  onExpand: () => void;
  onRefresh: () => void;
  refreshBusy: boolean;
};

type WidgetScreen = 'usage' | 'activity';

const WidgetMode = ({
  snapshots,
  statuses,
  modelBreakdowns,
  recentActivity,
  onExpand,
  onRefresh,
  refreshBusy,
}: WidgetModeProps) => {
  const [screen, setScreen] = useState<WidgetScreen>('usage');
  const [activityProvider, setActivityProvider] = useState<ProviderId>('codex');
  const authStates = useUsageStore((s) => s.authStates);

  useEffect(() => {
    if (recentActivity[activityProvider]?.length > 0) return;
    const fallback = providers.find((provider) => recentActivity[provider]?.length > 0);
    if (fallback) {
      setActivityProvider(fallback);
    }
  }, [activityProvider, recentActivity]);

  const openActivity = (provider: ProviderId) => {
    setActivityProvider(provider);
    setScreen('activity');
  };

  return (
    <div className="qp-shell">
      <div className="nav-bar" data-tauri-drag-region style={{ flexShrink: 0 }}>
        <div className="nav-header" data-tauri-drag-region>
          {/* Left: logo + title + refresh */}
          <div className="nav-header-left" data-tauri-drag-region>
            <img src="/quotapulse-mark.svg" alt="QuotaPulse" className="nav-logo" />
            <span className="nav-title" data-tauri-drag-region>QuotaPulse</span>
            <button
              className="nav-action-btn"
              onClick={onRefresh}
              disabled={refreshBusy}
              title="Actualizar todo (Ctrl+R)"
            >
              <Refresh size={14} className={refreshBusy ? 'spin-icon' : ''} />
            </button>
          </div>

          {/* Center: screen toggle pills */}
          <nav className="nav-pill-bar" data-tauri-drag-region>
            <button
              className={`nav-pill ${screen === 'usage' ? 'nav-pill-active' : ''}`}
              onClick={() => setScreen('usage')}
              title="Anillos de uso"
            >
              <Gauge size={14} />
            </button>
            <button
              className={`nav-pill ${screen === 'activity' ? 'nav-pill-active' : ''}`}
              onClick={() => setScreen('activity')}
              title="Conversaciones recientes"
            >
              <History size={14} />
            </button>
          </nav>

          {/* Right: expand + window controls */}
          <div className="nav-header-right">
            <button
              className="nav-action-btn"
              onClick={onExpand}
              title="Expandir a vista completa"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                <path d="M1 9L9 1M9 1H3.5M9 1V6.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            <div className="nav-traffic-lights">
              <button type="button" aria-label="Minimizar" title="Minimizar" className="window-btn window-btn-minimize"
                onClick={() => { if (isTauriRuntime()) getCurrentWindow().minimize(); }}
              />
              <button type="button" aria-label="Cerrar" title="Cerrar" className="window-btn window-btn-close"
                onClick={() => { if (isTauriRuntime()) invoke('quit_app'); }}
              />
            </div>
          </div>
        </div>
      </div>

      {screen === 'usage' ? (
        <div className="qp-widget-usage qp-scroll">
          <InkBlock snapshots={snapshots} providers={providers} compact />
          <ul className="qp-wrows" aria-label="Cuota por proveedor">
            {providers.map((id) => {
              const snapshot = snapshots[id];
              const access = getProviderAccessState(statuses[id], snapshot, authStates[id]);
              const [primary] = displayWindows(snapshot);
              const pct = Math.max(0, Math.min(100, primary?.utilization ?? 0));
              // No real figure → the status word, never an empty bar or 0 %.
              const emptyLabel = snapshot ? access.label : access.health === 'error' ? 'No disponible' : 'En espera';
              return (
                <li key={id}>
                  <button type="button" className="qp-wrow" onClick={() => openActivity(id)} title={access.detail}>
                    <ProviderMark provider={id} size={16} />
                    <span className="qp-wrow-name">{meta[id].name}</span>
                    {primary ? (
                      <>
                        <span className="qp-wrow-bar" aria-hidden="true">
                          <span style={{ width: `${pct}%`, background: barColor(primary) }} />
                        </span>
                        <span className="qp-wrow-win">{widgetWindowLabel(primary.window_type)}</span>
                        <span className="qp-wrow-pct">{formatWindowPercent(primary)}</span>
                      </>
                    ) : (
                      <span className="qp-wrow-empty" style={{ color: access.color }}>{emptyLabel}</span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      ) : (
        <WidgetActivityView
          provider={activityProvider}
          statuses={statuses}
          snapshots={snapshots}
          modelBreakdowns={modelBreakdowns}
          recentActivity={recentActivity}
          onSelectProvider={setActivityProvider}
        />
      )}
    </div>
  );
};

export default WidgetMode;
