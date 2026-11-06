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
  { label: 'Proveedores', value: 'Claude, ChatGPT, Gemini, Cursor', icon: Cpu },
  { label: 'Vistas', value: 'Widget + Panel', icon: LayoutDashboard },
  { label: 'Cobertura', value: 'Uso, prompts, estado', icon: Activity },
  { label: 'Versión', value: `v${APP_VERSION}`, icon: Package },
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
      setState(`Instalando v${update.version}…`);
      try {
        await installUpdate(update, (pct) => setState(`Instalando v${update.version}${pct != null ? ` · ${pct}%` : '…'}`));
      } catch (e) {
        setState(`Error al actualizar: ${String(e)}`);
      }
      return;
    }
    setState('Buscando…');
    const result = await checkForUpdate();
    if (result.kind === 'available') {
      setUpdate(result.update);
      setState(`v${result.update.version} disponible`);
    } else if (result.kind === 'none') {
      setState('Tienes la última versión');
    } else {
      setState('Aún no hay ninguna versión publicada');
    }
  };

  return (
    <>
      <GlassButton variant="default" size="sm" onClick={run} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}>
        <RefreshCw size={12} />
        {update ? `Instalar v${update.version} y reiniciar` : 'Buscar actualizaciones'}
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
        <span className="stg-badge">Widget + Panel</span>
        <span className="stg-badge">Historial de CLI</span>
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
      <span className="abt-section-title">Proyecto</span>
      <span className="abt-maintainer">Desarrollado por {APP_AUTHOR}</span>
      <p className="abt-project-desc">
        Código abierto. Las credenciales no salen de este equipo; las consultas de cuota van directas a cada proveedor.
      </p>
      <div className="abt-version-row">
        <div className="abt-version-cell">
          <span className="abt-version-label">Versión</span>
          <span className="abt-version-value">{APP_VERSION}</span>
        </div>
        <div className="abt-version-cell">
          <span className="abt-version-label">Licencia</span>
          <span className="abt-version-value">MIT</span>
        </div>
      </div>
      {APP_REPO_URL && (
        <GlassButton variant="primary" size="sm" onClick={openRepo} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}>
          <Github size={12} />
          Repositorio en GitHub
        </GlassButton>
      )}
      <UpdateCheckButton />
      <GlassButton variant="default" size="sm" onClick={openLogFolder} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}>
        <FolderOpen size={12} />
        Abrir carpeta de registros
      </GlassButton>
    </div>
  </div>
);

export default AboutPanel;
