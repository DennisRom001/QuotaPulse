<div align="center">

<img src="./public/quotapulse-icon.png" alt="QuotaPulse" width="112" />

# QuotaPulse

**Real-time AI quota monitor for Windows — every plan's remaining usage and reset time, at a glance.**
Claude Code · ChatGPT / Codex CLI · Gemini · Cursor · LiteLLM Proxy

**English** · [Español](README.es.md)

[![License: MIT](https://img.shields.io/badge/license-MIT-111111.svg)](LICENSE)
[![Latest release](https://img.shields.io/github/v/release/DennisRom001/QuotaPulse?color=111111)](https://github.com/DennisRom001/QuotaPulse/releases/latest)
[![Platform: Windows 10/11](https://img.shields.io/badge/platform-Windows%2010%2F11-111111.svg)](#installation-windows)
[![Built with Tauri 2](https://img.shields.io/badge/built%20with-Tauri%202-111111.svg)](https://tauri.app)

</div>

<!--
  DEMO — add a GIF or screenshot here, then uncomment the line below.
  Suggested path: docs/demo.gif (about 900 px wide).

  <p align="center"><img src="docs/demo.gif" alt="QuotaPulse demo" width="720" /></p>
-->

---

If you use several AI coding tools at once, you have probably been cut off mid-task because a 5-hour window ran out, or opened three dashboards just to see where you stand. QuotaPulse puts every plan in one small window — or a floating capsule at the top of your screen — so you always know how much is left and when it resets.

- **Native and light.** Tauri 2 + Rust + React. The Rust core uses about 30 MB of memory; the UI runs on the system's WebView2 runtime instead of a bundled Chromium.
- **100% local, no telemetry.** It reuses the sessions your CLIs already have on this machine and talks **directly** to each provider's official API. No account, no QuotaPulse server, no middleman.
- **Real numbers only.** A percentage on screen is exactly what the provider reported. When there is no live figure, QuotaPulse says so instead of drawing a fake 0% bar.

## Supported providers

| Provider | What QuotaPulse shows | Where it comes from |
|---|---|---|
| **Claude Code** | 5-hour and 7-day windows, live | `api.anthropic.com`, using Claude Code's own login |
| **ChatGPT / Codex CLI** | Session and weekly usage, live | `chatgpt.com`, using the `codex login` session |
| **Gemini** | Antigravity (live) → Gemini CLI quota → Google AI Studio key → Gemini desktop app | Local Antigravity server, Google Code Assist, AI Studio |
| **Cursor** | Plan usage when the editor session is readable; otherwise local activity and a calibrated estimate (always marked `~`) | `cursor.com` and Cursor's local state, read-only |
| **LiteLLM Proxy** | Tokens, spend and budgets per provider (`LIVE — LITELLM`) | Your local proxy on `127.0.0.1` |

## Key features

- **Floating capsule.** A compact ink pill that stays on top near the top of the screen, with one monogram per AI, its status dot and a thin bar only when a real figure exists. Drag it anywhere; it remembers its position. Click it to show or hide the main window. While it is on, the main window stays off the taskbar. It can be turned off in Settings.
- **Clean, low-noise design.** Warm paper surfaces, white cards, one ink block for the quota closest to its limit, 4 px progress bars and tabular figures. Light and dark themes, a custom thin-stroke icon set and geometric provider monograms. Built for a 360 px window, with a compact widget mode.
- **Honest states.** `LIVE`, `ACTIVE`, `LOCAL — AUTH EXPIRED` and so on are shown as reported. Estimates carry a `~` and use a neutral grey; bars turn amber at 70% and red at 90%.
- **Real time without burning resources.** One native, debounced file watcher, and at most one live API call per provider every 30 seconds.
- **Threshold alerts and limit forecasts.** Desktop notifications when a quota crosses your thresholds, plus an estimate of when it will run out.
- **Budgets and history.** Estimated tokens and cost from the local logs of Claude Code, Codex CLI and Gemini CLI, kept for up to 90 days, with project, session and provider-comparison views.
- **Export.** CSV, JSON, or an HTML report ready for *Print to PDF*.
- **Desktop citizen.** Tray icon, start with Windows, one-click Claude Code sign-in (opens `claude auth login` in a console) and signed automatic updates.
- **Interface language:** the UI is currently in Spanish.

## Privacy & security

- **Read-only credentials.** QuotaPulse reads each tool's existing login files and never modifies, refreshes or stores your credentials.
- **Closed HTTPS allowlist.** Network calls go only to official hosts — `api.anthropic.com`, `chatgpt.com`, `cloudcode-pa.googleapis.com`, `generativelanguage.googleapis.com` and `cursor.com` — over HTTPS, with proxies and redirects disabled.
- **Local services stay local.** Antigravity and LiteLLM are only contacted on `127.0.0.1`.
- **Signed updates.** Updates come from GitHub Releases, and every package is verified against the public key built into the app before it is installed. The startup check can be turned off in Settings.
- **No telemetry, no analytics.**

## Installation (Windows)

1. Open the [latest release](https://github.com/DennisRom001/QuotaPulse/releases/latest).
2. Download **`QuotaPulse_x.y.z_x64-setup.exe`** (or the `.msi`).
3. Run it. No Node.js or Rust needed; WebView2 is included.

**Requirements:** Windows 10 or 11. Install whichever tools you want to monitor: Claude Code, Codex CLI, Gemini CLI, Antigravity or Cursor. Later versions install themselves (Settings → About → *Buscar actualizaciones*).

## Optional: LiteLLM Proxy

`start-litellm.bat` creates an isolated `.venv` next to the script, installs pinned, hash-verified `litellm[proxy]` packages and starts the proxy on `127.0.0.1:4000`. QuotaPulse launches it automatically and windowless when the port is closed, using a random per-install master key stored in Windows Credential Manager. Requires Python 3.10–3.14. Set `QP_DISABLE_LITELLM=1` to turn this off.

For LiteLLM to report spend per provider it needs configured models and a database — see the LiteLLM documentation.

## Configuration

Every setting is optional and documented in [`.env.example`](.env.example) (names only, never values). On Windows, QuotaPulse also reads your user environment, so `setx` works without restarting the app:

```powershell
setx QP_DISABLE_LITELLM 1
```

## Build from source

### Prerequisites

- **Node.js 20+** and npm
- **Rust** (stable, MSVC target) — install it from [rustup.rs](https://rustup.rs/)
- **Visual Studio Build Tools** with the *Desktop development with C++* workload

### Steps

```bash
git clone https://github.com/DennisRom001/QuotaPulse.git
```

```bash
cd QuotaPulse
```

```bash
npm install
```

```bash
npm run tauri build
```

The executable and installers land in `src-tauri/target/release/`. Use `npm run tauri dev` for development. Signing, releasing and useful paths are in [BUILDING.md](BUILDING.md); the architecture is in [ARCHITECTURE.md](ARCHITECTURE.md).

## License

[MIT](LICENSE).
