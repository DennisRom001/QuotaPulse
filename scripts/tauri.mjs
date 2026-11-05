// Thin wrapper around the Tauri CLI used by `npm run tauri …`.
//
// Tauri signs the main binary (via bundle.windows.signCommand) while it builds
// the installers, but leaves the loose `target/release/<app>.exe` unsigned
// afterwards. After a successful `tauri build` on Windows this wrapper signs
// that file with the same script, so every artifact in target/release is
// signed. All other commands are passed straight through.
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);

// Call the CLI's JS entry with the current Node binary: no shell involved, so
// paths with spaces (e.g. "Monitoreo de IA local") are passed through intact.
const tauriCli = join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
const run = spawnSync(process.execPath, [tauriCli, ...args], { stdio: 'inherit', cwd: root });
if (run.status !== 0) process.exit(run.status ?? 1);

const isBuild = args[0] === 'build' && !args.includes('--debug');
if (process.platform === 'win32' && isBuild) {
  const conf = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
  const exe = join(root, 'src-tauri', 'target', 'release', `${conf.mainBinaryName ?? conf.productName}.exe`);
  if (existsSync(exe)) {
    const sign = spawnSync(
      'powershell',
      ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', join('scripts', 'sign-windows.ps1'), exe],
      { stdio: 'inherit', cwd: join(root, 'src-tauri') },
    );
    if (sign.status !== 0) process.exit(sign.status ?? 1);
  }
}
