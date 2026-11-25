import { Activity, Chip, FolderOpen, ExternalLink, Layout, Package, Refresh } from '@/components/icons';
import { openUrl } from '@tauri-apps/plugin-opener';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import type { Update } from '@tauri-apps/plugin-updater';
import { checkForUpdate, installUpdate } from '@/utils/updater';
import GlassButton from '@/components/glass/GlassButton';
import { APP_AUTHOR, APP_NAME, APP_REPO_URL } from '@/constants/appMeta';
import { useAppVersion } from '@/hooks/useAppVersion';
import { isTauriRuntime } from '@/utils/runtime';

const signalCards = (version: string) =>
  [
    { label: 'Proveedores', value: 'Claude, ChatGPT, Gemini, Cursor', icon: Chip },
    { label: 'Vistas', value: 'Widget + Panel', icon: Layout },
    { label: 'Cobertura', value: 'Uso, prompts, estado', icon: Activity },
    { label: 'Versión', value: `v${version}`, icon: Package },
  ] as const;

const clock = () => new Date().toLocaleTimeString('es', { hour: '2-digit', minute: '2-digit' });

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

type CheckState = { text: string; tone: 'muted' | 'error'; detail?: string };

const UpdateCheckButton = ({ version }: { version: string }) => {
  const [state, setState] = useState<CheckState>();
  const [update, setUpdate] = useState<Update>();
  const [busy, setBusy] = useState(false);

  const run = async () => {
    if (busy) return;
    if (update) {
      setBusy(true);
      setState({ text: `Instalando v${update.version}…`, tone: 'muted' });
      try {
        await installUpdate(update, (pct) =>
          setState({ text: `Instalando v${update.version}${pct != null ? ` · ${pct}%` : '…'}`, tone: 'muted' }),
        );
      } catch (e) {
        // Download, signature or installer failures land here with their reason.
        setState({ text: `No se pudo instalar v${update.version}: ${String(e)}`, tone: 'error', detail: String(e) });
      } finally {
        setBusy(false);
      }
      return;
    }
    setBusy(true);
    setState({ text: 'Buscando…', tone: 'muted' });
    const result = await checkForUpdate();
    setBusy(false);
    if (result.kind === 'available') {
      setUpdate(result.update);
      setState({ text: `v${result.update.version} disponible (tienes v${version})`, tone: 'muted' });
    } else if (result.kind === 'none') {
      setState({ text: `Tienes la última versión (v${version}) · comprobado a las ${clock()}`, tone: 'muted' });
    } else if (result.kind === 'unavailable') {
      setState({ text: 'Las actualizaciones solo se comprueban en la app instalada.', tone: 'muted' });
    } else {
      setState({ text: result.message, tone: 'error', detail: result.detail });
    }
  };

  return (
    <>
      <GlassButton
        variant="default"
        size="sm"
        onClick={run}
        disabled={busy}
        style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}
      >
        <Refresh size={12} className={busy ? 'spin-icon' : undefined} />
        {update ? `Instalar v${update.version} y reiniciar` : 'Buscar actualizaciones'}
      </GlassButton>
      {state && (
        <span
          className={`abt-update-state ${state.tone === 'error' ? 'is-error' : ''}`}
          role={state.tone === 'error' ? 'alert' : 'status'}
          title={state.detail}
        >
          {state.text}
        </span>
      )}
    </>
  );
};

const AboutPanel = () => {
  const version = useAppVersion();
  return (
  <div className="abt-root">
    {/* Hero */}
    <div className="abt-hero">
      <div className="abt-logo-frame">
        <img src="/quotapulse-mark.svg" alt="QuotaPulse" className="abt-logo" />
      </div>
      <div className="abt-hero-copy">
        <span className="abt-app-name">{APP_NAME}</span>
        <span className="stg-badge">v{version}</span>
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
      {signalCards(version).map((card) => {
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
          <span className="abt-version-value">{version}</span>
        </div>
        <div className="abt-version-cell">
          <span className="abt-version-label">Licencia</span>
          <span className="abt-version-value">MIT</span>
        </div>
      </div>
      {APP_REPO_URL && (
        <GlassButton variant="primary" size="sm" onClick={openRepo} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}>
          <ExternalLink size={12} />
          Repositorio en GitHub
        </GlassButton>
      )}
      <UpdateCheckButton version={version} />
      <GlassButton variant="default" size="sm" onClick={openLogFolder} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, alignSelf: 'center' }}>
        <FolderOpen size={12} />
        Abrir carpeta de registros
      </GlassButton>
    </div>
  </div>
  );
};

export default AboutPanel;
