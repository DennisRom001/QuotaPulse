# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(Tauri 2 desktop app: the interface is a web view rendered inside a Windows window and tray widget. The design language is web, not native iOS/Android.)

## Users

Developers in general — public, open-source tool — who work on Windows with several AI coding subscriptions and CLIs at the same time (Claude Code, Codex CLI, Gemini CLI / Antigravity, Cursor, optionally a local LiteLLM proxy). Their job: know at a glance how much of each plan is left and when it resets, so they can pick the right tool and never get cut off in the middle of a task.

## Product Purpose

QuotaPulse shows, in one small always-available window and in the Windows tray, the live quota and usage of every AI provider the developer already uses. Success means the developer never has to open a provider dashboard to know where they stand, is warned before a limit is reached, and can look back at tokens and cost over time.

## Positioning

- **Everything at a glance:** all providers' quotas together, always visible from the tray, updated in real time.
- **Local and private:** no QuotaPulse account, server or telemetry. It reuses the sessions the developer's CLIs already have on the machine and talks directly to each provider's official API (exact HTTPS host allowlist).
- **Warnings before the limit:** notifications on threshold crossings and a forecast of when a quota will run out.
- **History and cost:** tokens, estimated cost, projects and sessions kept locally for up to 90 days, beyond what each provider's dashboard shows.

## Operating Context

- Runs on the developer's own Windows 11 machine, usually starting with Windows and living in the tray; the main window opens on demand.
- Depends on CLIs being installed and signed in: Claude Code (`claude auth login`), Codex CLI (`codex login`), Gemini CLI / Antigravity, Cursor. When a session is missing or expired, the app says so and offers the way back (e.g. a one-click Claude sign-in that opens a terminal).
- Optional local LiteLLM proxy on 127.0.0.1:4000, auto-started by the app with a per-install key.
- Distributed through GitHub Releases (NSIS/MSI) with a signed auto-updater. The maintainer's own machine runs Smart App Control, so local native builds are not runnable; builds and releases happen in GitHub Actions.

## Capabilities and Constraints

- Live quota windows per provider (e.g. Claude 5-hour and 7-day), plan label, reset countdowns, tray badges with usage rings, threshold notifications, limit forecasts, budgets, per-provider refresh cadence, CSV/JSON/PDF export, projects/sessions views, provider comparison, compact widget mode, light/dark/system themes and a glass-opacity setting.
- **Interface in Spanish:** all UI text is Spanish. (Some backend-generated status notes are still English; new copy is Spanish.)
- **Windows first:** Windows 11 is the target platform (tray, Credential Manager, NSIS/MSI installer). macOS and Linux are not priorities.
- **Small window:** must work well from about 360 px wide as a tray widget, while remaining freely resizable larger.
- **No invented data:** never present an estimated or derived percentage as a real quota. When no live figure exists, say so plainly (e.g. local token totals without a percentage). Estimates that exist on purpose (Cursor's calibrated estimate) are always labelled as estimates.
- Terminology in use: cuota, ventana (5 h / 7 d), reinicio, plan, sesión, en vivo (LIVE), activo (ACTIVE), local, estimación.

## Brand Commitments

- Name: **QuotaPulse**. App icon: `public/quotapulse-mark.svg` (source; `public/quotapulse-icon.png` is its 256 px raster for the tray and README, and `src-tauri/icons/` is generated from it with `npx tauri icon`). Author: Dennis Romero (GitHub DennisRom001). License: MIT.
- Provider names and logos (Claude, ChatGPT, Gemini, Cursor, LiteLLM) are third-party marks shown only to identify each provider (`public/providers/`).
- Voice: plain, direct Spanish; states facts and next steps (what happened, what to do), without hype.

## Evidence on Hand

- Public repository and signed releases: https://github.com/DennisRom001/QuotaPulse (v1.0.0 → v1.2.3).
- README (Spanish) describing features and data sources; app icon and provider logos in `public/`.
- No testimonials, user counts, download numbers, press or benchmarks exist; future work must not fabricate any.

## Product Principles

1. **Truth over polish:** a number on screen is either real or clearly marked as an estimate or local total; never fill gaps with plausible-looking figures.
2. **Glanceable first:** the most important answer — how much is left and when it resets — must be readable in a second from the tray or the smallest window.
3. **Private by construction:** nothing leaves the machine except direct calls to each provider's official API; no account, server or telemetry of our own.
4. **Tell the user what to do:** when data is missing, stale or a session ended, explain why and offer the next step instead of failing silently.
5. **Light and native to the desktop:** a small, resource-frugal tray companion that respects Windows conventions.
