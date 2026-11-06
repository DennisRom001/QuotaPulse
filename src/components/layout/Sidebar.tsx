import { GitCompareArrows, FolderKanban, Home, RefreshCw, Settings2 } from 'lucide-react';
import { Fragment, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import ProviderLogo from '@/components/providers/ProviderLogo';
import { PageId, ProviderId } from '@/types';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUsageStore } from '@/stores/usageStore';
import { getProviderAccessState, providerAccessDotClass } from '@/utils/providerAccess';
import { isTauriRuntime } from '@/utils/runtime';
import { SOURCE_COLORS } from '@/utils/sources';
import { useLitellmStatus } from '@/hooks/useLitellmStatus';

type NavBarProps = {
  activePage: PageId;
  onNavigate: (page: PageId) => void;
  onRefresh: () => void;
  refreshBusy: boolean;
  onWidget: () => void;
};

const providers: { id: ProviderId; label: string; tint: string }[] = [
  { id: 'claude', label: 'Claude', tint: 'claude' },
  { id: 'codex', label: 'ChatGPT', tint: 'codex' },
  { id: 'antigravity', label: 'Gemini', tint: 'antigravity' },
  { id: 'cursor', label: 'Cursor', tint: 'cursor' },
];

const NavBar = ({ activePage, onNavigate, onRefresh, refreshBusy, onWidget }: NavBarProps) => {
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const statuses = useUsageStore((s) => s.statuses);
  const snapshots = useUsageStore((s) => s.snapshots);
  const authStates = useUsageStore((s) => s.authStates);
  const litellm = useLitellmStatus();
  const geminiSource = useSettingsStore((s) => s.geminiSource);
  const setGeminiSource = useSettingsStore((s) => s.setGeminiSource);
  const geminiSources = snapshots.antigravity?.sources ?? [];
  const antigravitySource = geminiSources.find((s) => s.id === 'antigravity');
  const hasDesktop = geminiSources.some((s) => s.id === 'desktop');

  // Toggle the Gemini card between Antigravity and the Gemini App.
  const toggleGeminiSource = () => {
    if (geminiSource === 'antigravity') setGeminiSource(hasDesktop ? 'desktop' : 'auto');
    else setGeminiSource('antigravity');
    if (activePage !== 'overview' && activePage !== 'antigravity') onNavigate('overview');
  };

  const enabledProvidersFiltered = useMemo(
    () => providers.filter(({ id }) => enabledProviders[id]),
    [enabledProviders]
  );

  return (
    <div className="nav-bar" data-tauri-drag-region>
      {/* Row 1: brand + actions + window controls */}
      <div className="nav-header" data-tauri-drag-region>
        <div className="nav-header-left" data-tauri-drag-region>
          <img src="/quotapulse-icon.png" alt="QuotaPulse" className="nav-logo" />
          <span className="nav-title" data-tauri-drag-region>QuotaPulse</span>
          {litellm && (
            <span
              className={`nav-litellm ${litellm.up ? 'is-up' : ''}`}
              title={
                litellm.up
                  ? `LiteLLM Proxy activo en ${litellm.base_url ?? '127.0.0.1:4000'} — ${
                      litellm.providers.length > 0
                        ? `datos de ${litellm.providers.length} proveedor(es)`
                        : 'aún sin tráfico enrutado'
                    }`
                  : 'LiteLLM Proxy no está en ejecución (se inicia en segundo plano si está disponible)'
              }
            >
              <span className="nav-source-dot" style={{ background: litellm.up ? SOURCE_COLORS.live : SOURCE_COLORS.off }} />
              LiteLLM
            </span>
          )}
        </div>

        <div className="nav-header-right">
          <button
            className="nav-action-btn"
            onClick={onRefresh}
            disabled={refreshBusy}
            title="Actualizar todo (Ctrl+R)"
          >
            <RefreshCw size={11} className={refreshBusy ? 'spin-icon' : ''} />
          </button>
          <button
            className="nav-action-btn"
            onClick={onWidget}
            title="Cambiar a vista widget"
          >
            <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
              <path d="M9 1H3.5M9 1V6.5M9 1L1 9" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" transform="rotate(180 5 5)"/>
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

      {/* Row 2: navigation — left pages | right providers + settings */}
      <nav className="nav-pill-row" data-tauri-drag-region>
        <div className="nav-pill-group nav-pill-group-left">
          <button
            className={`nav-pill ${activePage === 'overview' ? 'nav-pill-active' : ''}`}
            onClick={() => onNavigate('overview')}
            title="Inicio"
          >
            <Home size={13} />
          </button>
          <button
            className={`nav-pill ${activePage === 'projects' ? 'nav-pill-active' : ''}`}
            onClick={() => onNavigate('projects')}
            title="Proyectos"
          >
            <FolderKanban size={13} />
          </button>
          {enabledProvidersFiltered.length >= 2 ? (
            <button
              className={`nav-pill ${activePage === 'comparison' ? 'nav-pill-active' : ''}`}
              onClick={() => onNavigate('comparison')}
              title="Comparar fuentes"
            >
              <GitCompareArrows size={13} />
            </button>
          ) : null}
        </div>

        <div className="nav-pill-group nav-pill-group-right">
          {enabledProvidersFiltered.map(({ id, label, tint }) => {
            const access = getProviderAccessState(statuses[id], snapshots[id], authStates[id]);
            const healthClass = providerAccessDotClass(access.health);
            return (
              <Fragment key={id}>
                <button
                  className={`nav-pill ${activePage === id ? `nav-pill-active nav-pill-tint-${tint}` : ''}`}
                  onClick={() => onNavigate(id)}
                  title={`${label}: ${access.detail}`}
                >
                  <ProviderLogo provider={id} size={14} />
                  <span className={`nav-pill-dot ${healthClass}`} />
                </button>
                {id === 'antigravity' && antigravitySource && (
                  <button
                    type="button"
                    className={`nav-source-chip ${geminiSource === 'antigravity' ? 'is-selected' : ''}`}
                    onClick={toggleGeminiSource}
                    aria-pressed={geminiSource === 'antigravity'}
                    title={`Antigravity: ${antigravitySource.badge}. Clic para mostrar ${
                      geminiSource === 'antigravity' ? (hasDesktop ? 'la app de Gemini' : 'la mejor fuente') : 'Antigravity'
                    } en la tarjeta de Gemini.`}
                  >
                    <span>AG</span>
                    <span
                      className="nav-source-dot"
                      style={{ background: SOURCE_COLORS[antigravitySource.state] }}
                    />
                  </button>
                )}
              </Fragment>
            );
          })}
          <button
            className={`nav-pill ${activePage === 'settings' ? 'nav-pill-active' : ''}`}
            onClick={() => onNavigate('settings')}
            title="Ajustes (Ctrl+,)"
          >
            <Settings2 size={13} />
          </button>
        </div>
      </nav>
    </div>
  );
};

export default NavBar;
