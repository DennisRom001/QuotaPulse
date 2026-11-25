// Signed auto-updates from GitHub Releases.
//
// The Tauri updater downloads `latest.json` from the endpoint configured in
// tauri.conf.json (DennisRom001/QuotaPulse releases), verifies the update's
// signature against the bundled public key, and installs it. This is the only
// network call QuotaPulse makes to GitHub; it can be turned off in Settings.
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { isTauriRuntime } from '@/utils/runtime';

export type UpdateCheck =
  | { kind: 'none' }
  | { kind: 'available'; update: Update }
  | { kind: 'unavailable' }
  | { kind: 'error'; message: string; detail: string };

/**
 * Turn an updater error into a short Spanish reason the user can act on.
 * The raw error is kept as `detail` so nothing is hidden.
 */
export const describeUpdateError = (error: unknown): string => {
  const raw = String(error);
  const text = raw.toLowerCase();
  // tauri-plugin-updater's ReleaseNotFound: every endpoint answered non-2xx or
  // with something that is not a release JSON.
  if (text.includes('valid release json')) {
    return 'GitHub no devolvió un latest.json válido: la release no existe, es un borrador o no se pudo descargar.';
  }
  if (text.includes('404') || text.includes('not found')) {
    return 'No se encontró latest.json en GitHub (404): la release no existe o no es pública.';
  }
  if (text.includes('signature') || text.includes('minisign') || text.includes('firma')) {
    return 'La firma de la actualización no coincide con la clave de la app: no se instaló nada.';
  }
  if (
    text.includes('error sending request') ||
    text.includes('dns') ||
    text.includes('connect') ||
    text.includes('timed out') ||
    text.includes('timeout') ||
    text.includes('network')
  ) {
    return 'No hay conexión con GitHub para comprobar actualizaciones.';
  }
  if (text.includes('json') || text.includes('parse') || text.includes('deserialize')) {
    return 'latest.json no tiene un formato válido.';
  }
  if (text.includes('platform') || text.includes('target')) {
    return 'latest.json no incluye una actualización para Windows x64.';
  }
  return `No se pudo comprobar: ${raw}`;
};

export const checkForUpdate = async (): Promise<UpdateCheck> => {
  if (!isTauriRuntime()) return { kind: 'unavailable' };
  try {
    const update = await check();
    return update ? { kind: 'available', update } : { kind: 'none' };
  } catch (error) {
    return { kind: 'error', message: describeUpdateError(error), detail: String(error) };
  }
};

/** Download, verify and install, then restart into the new version. */
export const installUpdate = async (update: Update, onProgress?: (pct: number | undefined) => void) => {
  let total = 0;
  let received = 0;
  await update.downloadAndInstall((event) => {
    if (event.event === 'Started') {
      total = event.data.contentLength ?? 0;
      onProgress?.(total ? 0 : undefined);
    } else if (event.event === 'Progress') {
      received += event.data.chunkLength;
      onProgress?.(total ? Math.min(100, Math.round((received / total) * 100)) : undefined);
    } else if (event.event === 'Finished') {
      onProgress?.(100);
    }
  });
  await relaunch();
};
