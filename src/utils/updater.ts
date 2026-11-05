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
  | { kind: 'error'; message: string };

export const checkForUpdate = async (): Promise<UpdateCheck> => {
  if (!isTauriRuntime()) return { kind: 'none' };
  try {
    const update = await check();
    return update ? { kind: 'available', update } : { kind: 'none' };
  } catch (error) {
    // Typical before the first release exists: latest.json returns 404.
    return { kind: 'error', message: String(error) };
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
