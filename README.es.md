<div align="center">

<img src="./public/quotapulse-icon.png" alt="QuotaPulse" width="112" />

# QuotaPulse

**Monitor de cuotas de IA en tiempo real para Windows**
Claude Code · ChatGPT / Codex CLI · Gemini · Cursor · LiteLLM Proxy

[English](README.md) · **Español**

</div>

---

QuotaPulse es una aplicación de escritorio ligera (Tauri 2 + Rust + React) que muestra en una sola ventana, o en una cápsula flotante en lo alto de la pantalla, cuánto te queda de cada plan de IA y cuándo se reinicia. Detecta automáticamente las sesiones que ya tienen tus herramientas en el equipo y consulta **directamente** a cada proveedor, sin servidores intermediarios ni telemetría.

## Características

- **Cuotas en vivo**
  - **Claude Code**: ventanas de 5 horas y 7 días desde `api.anthropic.com`, con el login de Claude Code (`~/.claude/.credentials.json`, almacén de credenciales del sistema o `CLAUDE_CODE_OAUTH_TOKEN`). El porcentaje se muestra tal como lo devuelve la API. Si la sesión caduca, el botón **«Iniciar sesión en Claude»** abre una consola con `claude auth login`.
  - **ChatGPT / Codex CLI**: uso de sesión y semanal desde `chatgpt.com`, con el login de `codex login`.
  - **Gemini**: cascada de fuentes — Antigravity (servidor local, LIVE) → cuota de Gemini CLI (Google Code Assist) → API key de Google AI Studio (ACTIVE) → app de escritorio de Gemini (ACTIVE). Puedes elegir la fuente desde la tarjeta.
  - **Cursor**: uso del plan desde `cursor.com` cuando la sesión del editor es legible; si no, actividad local (prompts y modelo) y una estimación calibrada, siempre marcada con `~`.
  - **LiteLLM Proxy**: si tienes un proxy LiteLLM local, sus métricas (tokens, gasto y presupuestos por proveedor) tienen prioridad y se marcan como `LIVE — LITELLM`.
- **Cápsula flotante**: una píldora pequeña, siempre visible en la parte superior de la pantalla, con un monograma por IA, su estado y una barra fina solo cuando hay una cifra real. Se arrastra y recuerda su posición; un clic muestra u oculta la ventana principal. Mientras está activa, la ventana no ocupa la barra de tareas. Se puede desactivar en Ajustes.
- **Diseño «Tinta sobre papel»**: papel cálido, tarjetas blancas, un bloque de tinta con la cuota más cercana al límite, barras de 4 px y cifras tabulares. Modo claro y oscuro (carbón neutro), iconos propios de trazo fino y monogramas geométricos para cada proveedor. Pensado para una ventana de 360 px, con modo widget compacto.
- **Datos honestos**: nunca muestra barras ni porcentajes inventados al 0 %. Sin cifra real verás el estado (`LIVE`, `ACTIVE`, `LOCAL — AUTH EXPIRED`…) y una nota; las estimaciones llevan `~` y van en gris.
- **Tiempo real con poco consumo**: un único vigilante de archivos nativo con *debounce* y como máximo una llamada en vivo a cada API cada 30 s.
- **Historial local**: tokens y coste estimados a partir de los logs de Claude Code, Codex CLI y Gemini CLI, guardados hasta 90 días; vistas de proyectos, sesiones y comparación entre proveedores.
- **Extras**: alertas por umbral, presupuestos, previsión de cuándo se agotará una cuota, exportación CSV / JSON / PDF (informe HTML listo para «Imprimir a PDF»), icono en la bandeja, inicio con Windows y actualizaciones automáticas firmadas.
- **Interfaz en español.**

## Privacidad y seguridad

- Las credenciales se **leen** de los archivos de cada herramienta; QuotaPulse nunca las modifica, renueva ni guarda.
- Todas las peticiones salen por HTTPS a una lista cerrada de dominios oficiales (`api.anthropic.com`, `chatgpt.com`, `cloudcode-pa.googleapis.com`, `generativelanguage.googleapis.com`, `cursor.com`), sin proxies ni redirecciones.
- Los servicios locales (Antigravity, LiteLLM) solo se usan en `127.0.0.1`.
- Actualizaciones automáticas firmadas desde GitHub Releases: cada paquete se verifica con la clave pública incluida en la app antes de instalarse. La comprobación al arrancar se puede desactivar en Ajustes.
- Sin telemetría ni analíticas.

## Requisitos

- Windows 10/11 (el instalador incluye WebView2).
- Opcional, según lo que quieras monitorizar: Claude Code, Codex CLI, Gemini CLI, Antigravity, Cursor.
- Opcional para LiteLLM: Python 3.10–3.14.

## Instalación

1. Ve a [Releases](https://github.com/DennisRom001/QuotaPulse/releases/latest).
2. Descarga el instalador **`QuotaPulse_x.y.z_x64-setup.exe`** (o el `.msi`).
3. Ejecútalo. No necesitas Node.js ni Rust.

Las versiones siguientes se instalan solas (Ajustes → Acerca de → «Buscar actualizaciones»).

## LiteLLM Proxy (opcional)

`start-litellm.bat` crea un entorno aislado `.venv` junto al script, instala versiones fijadas y verificadas por hash de `litellm[proxy]` y arranca el proxy en `127.0.0.1:4000`. QuotaPulse lo ejecuta automáticamente y sin ventanas al abrirse si el puerto está cerrado, con una clave maestra aleatoria por instalación guardada en el Administrador de credenciales de Windows (desactívalo con `QP_DISABLE_LITELLM=1`).
Para que LiteLLM registre gasto por proveedor necesita modelos configurados y una base de datos; consulta la documentación de LiteLLM.

## Configuración

Todas las variables son opcionales y están documentadas en [`.env.example`](.env.example) (solo nombres, sin valores). En Windows también se leen del entorno de usuario, así que `setx` funciona sin reiniciar la app:

```powershell
setx QP_DISABLE_LITELLM 1
```

## Compilar desde el código

### Prerrequisitos

- **Node.js 20+** y npm.
- **Rust** estable con el target MSVC (instálalo desde [rustup.rs](https://rustup.rs/)).
- **Visual Studio Build Tools** con la carga de trabajo «Desarrollo para el escritorio con C++».

### Pasos

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

El ejecutable y los instaladores quedan en `src-tauri/target/release/`. Para desarrollo usa `npm run tauri dev`. Firma, publicación de versiones y rutas útiles en [BUILDING.md](BUILDING.md); arquitectura en [ARCHITECTURE.md](ARCHITECTURE.md).

## Licencia

[MIT](LICENSE).
