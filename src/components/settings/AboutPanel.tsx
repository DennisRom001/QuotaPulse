import { Activity, Cpu, FolderOpen, Github, LayoutDashboard, Package, RefreshCw } from 'lucide-react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import type { Update } from '@tauri-apps/plugin-updater';
import { checkForUpdate, installUpdate } from '@/utils/updater';
import GlassButton from '@/components/glass/GlassButton';
import { APP_AUTHOR, APP_NAME, APP_REPO_URL, APP_VERSION } from '@/constants/appMeta';
import { isTauriRuntime } from '@/utils/runtime';

const signalCards = [
  { label: 'Providers', value: 'Claude, ChatGPT, Gemini, Cursor', icon: Cpu },
  { label: 'Surfaces', value: 'Widget + Dashboard', icon: LayoutDashboard },
  { label: 'Coverage', value: 'Usage, prompts, health', icon: Activity },
  { label: 'Version', value: `v${APP_VERSION}`, icon: Package },
] as const;

const openRepo = async () => {
  try {
    if (isTauriRuntime()) {
      await openUrl(APP_REPO_URL);
      return;
    }
  } catch (error) {
    console.error('failed to open repo url', error);
  }
  window.open(APP_REPO_URL, '_blank', 'noopener,noreferrer');
};

// Reveals the rolling log directory so users can attach logs to a bug report
// without hunting through app-data paths — the key diagnostic for a blank
// window or a startup crash.
const openLogFolder = async () => {
  if (!isTauriRuntime()) return;
  try {
    await invoke('open_log_directory');
  } catch (error) {
    console.error('failed to open log directory', error);
  }
};

const UpdateCheckButton = () => {
  const [state, setState] = useState<string>();
  const [update, setUpdate] = useState<Update>();

  const run = async () => {
    if (update) {
      setState(`Installing v${update.version}…`);
      try {
        await installUpdate(update, (pct) => setState(`Installing v${update.version}${pct != null ? ` · ${pct}%` : '…'}`));
      } catch (e) {
        setState(`Update failed: ${String(e)}`);
      }
      return;
    }
    setState('Checking…');
    const result = await checkForUpdate();
    if (result.kind === 'available') {
      setUpdate(result.update);
      setState(`v${result.update.version} is available`);
    } else if (result.kind === 'none') {
      setState('You are on the latest version');
    } else {
      setState('No published release found yet');
    }
  };

  return (
    <>
      <GlassButton variant="default" size="sm" onClick={run} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}>
        <RefreshCw size={12} />
        {update ? `Install v${update.version} & restart` : 'Check for updates'}
      </GlassButton>
      {state && <span className="abt-update-state">{state}</span>}
    </>
  );
};

const AboutPanel = () => (
  <div className="abt-root">
    {/* Hero */}
    <div className="abt-hero">
      <div className="abt-logo-frame">
        <img src="/quotapulse-icon.png" alt="QuotaPulse" className="abt-logo" />
      </div>
      <div className="abt-hero-copy">
        <span className="abt-app-name">{APP_NAME}</span>
        <span className="stg-badge">v{APP_VERSION}</span>
      </div>
      <p className="abt-desc">
        Monitor de uso para Claude, ChatGPT, Gemini y Cursor. Detecta las sesiones locales
        y consulta directamente los servidores oficiales. Sin intermediarios ni telemetría.
      </p>
      <div className="abt-tags">
        <span className="stg-badge">Conexión directa</span>
        <span className="stg-badge">Widget + Dashboard</span>
        <span className="stg-badge">CLI History</span>
      </div>
    </div>

    {/* Signal cards */}
    <div className="abt-signals">
      {signalCards.map((card) => {
        const Icon = card.icon;
        return (
          <div key={card.label} className="abt-signal">
            <span className="abt-signal-icon"><Icon size={13} strokeWidth={2.2} /></span>
            <span className="abt-signal-label">{card.label}</span>
            <span className="abt-signal-value">{card.value}</span>
          </div>
        );
      })}
    </div>

    {/* Project info */}
    <div className="abt-project-card">
      <span className="abt-section-title">Project</span>
      <span className="abt-maintainer">Developed by {APP_AUTHOR}</span>
      <p className="abt-project-desc">
        Open source. Credentials stay on this machine; quota calls go straight to each provider.
      </p>
      <div className="abt-version-row">
        <div className="abt-version-cell">
          <span className="abt-version-label">Version</span>
          <span className="abt-version-value">{APP_VERSION}</span>
        </div>
        <div className="abt-version-cell">
          <span className="abt-version-label">Track</span>
          <span className="abt-version-value">Open source</span>
        </div>
      </div>
      {APP_REPO_URL && (
        <GlassButton variant="primary" size="sm" onClick={openRepo} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}>
          <Github size={12} />
          GitHub Repo
        </GlassButton>
      )}
      <UpdateCheckButton />
      <GlassButton variant="default" size="sm" onClick={openLogFolder} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}>
        <FolderOpen size={12} />
        Open log folder
      </GlassButton>
    </div>
  </div>
);

export default AboutPanel;
