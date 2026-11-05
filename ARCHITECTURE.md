# Arquitectura de QuotaPulse

QuotaPulse es una app Tauri 2: un backend en Rust (`src-tauri/src`) que obtiene y guarda los datos,
y un frontend React + TypeScript (`src/`) que solo los muestra. El frontend nunca habla con los
proveedores; usa comandos Tauri (`invoke`) y el evento `usage-updated`.

```
 Disparadores                    Backend (Rust)                              Frontend (React)
 ───────────                     ──────────────                              ────────────────
 file_watcher (debounce) ─┐
 poll_scheduler (5 s tick)├─► aggregator ─► providers/* ─► SQLite store ─► usage-updated ─► stores (zustand)
 UI / bandeja ────────────┘        │                                            │
                                   └─► post_refresh: bandeja + alertas          └─► tarjetas, widget, ajustes
```

## Proveedores (`src-tauri/src/providers/`)

Cada proveedor implementa `UsageProvider` (`fetch_usage`, `fetch_cost_history`, `check_status`,
`compute_auth_state`). Orden de fuentes de cada uno:

| Proveedor | Fuentes, de mayor a menor prioridad |
|---|---|
| `claude/` | LiteLLM → API `api.anthropic.com` (`oauth_fetcher.rs`, credenciales en `keychain.rs`) → logs de Claude Code |
| `codex/` | LiteLLM → API `chatgpt.com` (`bearer_fetcher.rs`, `~/.codex/auth.json`) → logs de Codex |
| `antigravity/` (Gemini) | LiteLLM → servidor local Antigravity (`live_fetcher.rs`) → Gemini CLI (`gemini_quota.rs`) → API key (`gemini_api_key.rs`) → app de escritorio (`gemini_desktop.rs`) → logs |
| `cursor/` | LiteLLM → `cursor.com/api/usage` (sesión del state DB de Cursor) → proceso `Cursor.exe` activo |

Módulos compartidos:

- `http.rs` — único cliente HTTPS: lista cerrada de dominios oficiales, sin proxy, sin redirecciones,
  un intento por llamada.
- `litellm_proxy.rs` — conector del proxy LiteLLM local (solo loopback), con caché compartida.
- `env.rs` — lectura de variables de entorno, incluido el entorno de usuario de Windows.
- `process.rs` — comprobación de procesos en ejecución (Toolhelp32).
- `LiveGate` (`mod.rs`) — último dato en vivo, espaciado mínimo de 30 s y pausa tras un fallo.

Los snapshots llevan `badge` (etiqueta elegida por el proveedor, p. ej. `ACTIVE — LOCAL SESSION`)
y `sources` (estado de cada fuente, usado por el selector Antigravity / Gemini App).

## Tiempo real

- `watchers/file_watcher.rs`: un vigilante nativo para todas las carpetas de logs y archivos de
  credenciales; agrupa escrituras (1,5 s, máx. 5 s) y refresca solo el proveedor afectado.
- `watchers/activity.rs`: marca de última actividad por proveedor; con una sesión de Claude activa,
  `poll_scheduler` refresca cada 30 s.
- `litellm_launcher.rs`: al abrir la app, si el puerto 4000 está cerrado ejecuta
  `start-litellm.bat` sin ventana.

## Persistencia

`usage/store.rs` guarda snapshots, histórico por ventana y costes diarios en SQLite
(`%APPDATA%\com.quotapulse.desktop\usage.db`). Los ajustes de la interfaz viven en el
almacenamiento local de la WebView.

## Frontend (`src/`)

- `App.tsx` — navegación y contenedor con scroll único.
- `components/providers/OverviewCard.tsx` — tarjeta de cada servicio (porcentaje, plan, badge,
  barras de 8 px, selector de fuente de Gemini).
- `components/layout/Sidebar.tsx` — barra superior: proveedores, chip de Antigravity, estado de LiteLLM.
- `utils/providerAccess.ts` — reglas de badge (`LIVE` verde, `ACTIVE` azul, `LOCAL — …`).
- `styles/quotapulse.css` — capa de maquetación (scroll, rejilla, tarjetas).
