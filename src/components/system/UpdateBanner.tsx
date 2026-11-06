import { useEffect, useState } from 'react';
import type { Update } from '@tauri-apps/plugin-updater';
import { useSettingsStore } from '@/stores/settingsStore';
import { checkForUpdate, installUpdate } from '@/utils/updater';

/** Delay before the background check so startup stays instant. */
const CHECK_DELAY_MS = 8000;

/**
 * Slim banner shown only when a newer signed release exists. Checks once per
 * launch (if enabled in Settings); errors such as "no release yet" are silent.
 */
const UpdateBanner = () => {
  const autoUpdateCheck = useSettingsStore((s) => s.autoUpdateCheck);
  const hydrated = useSettingsStore((s) => s.hydrated);
  const [update, setUpdate] = useState<Update>();
  const [dismissed, setDismissed] = useState(false);
  const [progress, setProgress] = useState<number | 'busy'>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!hydrated || !autoUpdateCheck) return;
    const timer = window.setTimeout(() => {
      checkForUpdate().then((result) => {
        if (result.kind === 'available') setUpdate(result.update);
      });
    }, CHECK_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [hydrated, autoUpdateCheck]);

  if (!update || dismissed) return null;

  const install = async () => {
    setError(undefined);
    setProgress('busy');
    try {
      await installUpdate(update, (pct) => setProgress(pct ?? 'busy'));
    } catch (e) {
      setProgress(undefined);
      setError(String(e));
    }
  };

  return (
    <div className="qp-update" role="status">
      <span className="qp-update-text">
        {error
          ? `Error al actualizar: ${error}`
          : progress !== undefined
            ? `Instalando v${update.version}${typeof progress === 'number' ? ` · ${progress}%` : '…'}`
            : `QuotaPulse v${update.version} disponible`}
      </span>
      {progress === undefined && (
        <>
          <button type="button" className="qp-btn qp-btn-accent" onClick={install}>
            Instalar y reiniciar
          </button>
          <button type="button" className="qp-btn" onClick={() => setDismissed(true)}>
            Más tarde
          </button>
        </>
      )}
    </div>
  );
};

export default UpdateBanner;
