import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { APP_VERSION } from '@/constants/appMeta';
import { isTauriRuntime } from '@/utils/runtime';

/**
 * The version of the running app, as Tauri reports it (the installed binary,
 * i.e. what the updater compares against). In a browser, or if the call
 * fails, it falls back to the package.json version of this build.
 */
export const useAppVersion = (): string => {
  const [version, setVersion] = useState(APP_VERSION);
  useEffect(() => {
    if (!isTauriRuntime()) return;
    let alive = true;
    getVersion()
      .then((v) => alive && setVersion(v))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);
  return version;
};
