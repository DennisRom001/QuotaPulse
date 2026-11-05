<div align="center">

<img src="./public/quotapulse-icon.png" alt="QuotaPulse" width="112" />

# QuotaPulse

**Monitor de cuotas de IA en tiempo real para Windows**
Claude · ChatGPT (Codex) · Gemini / Antigravity · Cursor · LiteLLM Proxy

</div>

---

QuotaPulse es una aplicación de escritorio ligera (Tauri + Rust + React, ~45 MB de RAM) que muestra
en una sola ventana cuánto te queda de cada plan de IA. Detecta automáticamente las sesiones que ya
tienen tus herramientas en el equipo y consulta **directamente** a cada proveedor, sin servidores
intermediarios ni telemetría.

## Características

- **Cuotas en vivo**
  - **Claude**: ventanas de 5 horas y 7 días desde `api.anthropic.com`, con el login de Claude Code
    (`~/.claude/.credentials.json`, almacén de credenciales del sistema o `CLAUDE_CODE_OAUTH_TOKEN`).
    El porcentaje se muestra tal como lo devuelve la API.
  - **ChatGPT / Codex CLI**: uso de sesión y semanal desde `chatgpt.com`, con el login de `codex login`.
  - **Gemini**: cascada de fuentes — Antigravity (servidor local, LIVE) → cuota de Gemini CLI
    (Google Code Assist) → API key de Google AI Studio (ACTIVE) → app de escritorio de Gemini (ACTIVE).
    En la tarjeta y en la barra superior puedes alternar entre Antigravity y Gemini App.
  - **Cursor**: detecta si `Cursor.exe` está abierto (`ACTIVE — LOCAL SESSION`) y consulta el uso del
    plan cuando la sesión es legible.
  - **LiteLLM Proxy**: si tienes un proxy LiteLLM local, sus métricas (tokens, gasto y presupuestos por
    proveedor) tienen prioridad y se marcan como `LIVE — LITELLM`.
- **Tiempo real con poco consumo**: un único vigilante de archivos nativo con *debounce*, sondeo de 30 s
  mientras hay una sesión de Claude activa y cadencia normal el resto del tiempo; como máximo una
  llamada a cada API cada 30 s.
- **Estimaciones locales**: tokens y coste a partir de los logs de Claude Code, Codex CLI y Gemini CLI.
- **Interfaz compacta**: diseñada para una ventana de 360 px, con modo widget, scroll fluido, barras de
  8 px y badges claros (`LIVE` en verde, `ACTIVE` en azul, `LOCAL — AUTH EXPIRED`, etc.). Nunca muestra
  barras inventadas al 0 %.
- **Extras**: alertas por umbral, presupuestos, previsión de límite, proyectos/sesiones, exportación
  CSV/JSON/PDF, icono en la bandeja e inicio con Windows.

## Privacidad y seguridad

- Las credenciales se **leen** de los archivos de cada herramienta; QuotaPulse nunca las modifica,
  renueva ni guarda.
- Todas las peticiones salen por HTTPS a una lista cerrada de dominios oficiales
  (`api.anthropic.com`, `chatgpt.com`, `cloudcode-pa.googleapis.com`,
  `generativelanguage.googleapis.com`, `cursor.com`), sin proxies ni redirecciones.
- Los servicios locales (Antigravity, LiteLLM) solo se usan en `127.0.0.1`.
- Actualizaciones automáticas firmadas desde GitHub Releases: una comprobación al
  arrancar, desactivable en Ajustes; cada paquete se verifica con la clave pública
  antes de instalarse.
- Sin telemetría ni analíticas.

## Requisitos

- Windows 10/11 (WebView2 incluido en el instalador).
- Opcional, según lo que quieras monitorizar: Claude Code, Codex CLI, Gemini CLI, Antigravity, Cursor.
- Opcional para LiteLLM: Python 3.10–3.14.

## Instalación

Compílalo desde el código (ver abajo). Cuando haya versiones publicadas, los instaladores
(`QuotaPulse_x.y.z_x64-setup.exe` o `.msi`) estarán en
[Releases](https://github.com/DennisRom001/QuotaPulse/releases).

## LiteLLM Proxy (opcional)

`start-litellm.bat` crea un entorno aislado `.venv` junto al script, instala una versión fijada de
`litellm[proxy]` y arranca el proxy en `127.0.0.1:4000`. QuotaPulse lo ejecuta automáticamente y
sin ventanas al abrirse si el puerto está cerrado (desactívalo con `QP_DISABLE_LITELLM=1`).
Para que LiteLLM registre gasto por proveedor necesita modelos configurados y una base de datos;
consulta la documentación de LiteLLM.

## Configuración

Todas las variables son opcionales y están documentadas en [`.env.example`](.env.example)
(solo nombres, sin valores). En Windows también se leen del entorno de usuario, así que
`setx NOMBRE valor` funciona sin reiniciar la app.

## Compilar desde el código

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

El ejecutable y los instaladores firmados quedan en `src-tauri/target/release/`. Detalles de
firma y requisitos en [BUILDING.md](BUILDING.md); arquitectura en [ARCHITECTURE.md](ARCHITECTURE.md).

## Licencia

[MIT](LICENSE).
