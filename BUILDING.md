# Compilar QuotaPulse

## Requisitos

- **Todos:** Node.js 20+ y npm, y Rust estable (`rustup`).
- **Windows 10/11:** el target MSVC y Visual Studio Build Tools con la carga de trabajo
  **Desarrollo para el escritorio con C++**.
- **macOS:** Xcode Command Line Tools (`xcode-select --install`). Para una app universal,
  `rustup target add aarch64-apple-darwin x86_64-apple-darwin`.
- **Linux (Debian/Ubuntu 22.04+):**

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libdbus-1-dev libssl-dev build-essential
```

## Desarrollo

```bash
npm install
```

```bash
npm run tauri dev
```

Solo el frontend (sin backend Rust, sin datos):

```bash
npm run dev
```

## Build de producción

```bash
npm run tauri build
```

Genera en `src-tauri/target/release/` (o `target/<triple>/release/` con `--target`):

| Sistema | Paquetes en `bundle/` | Comando |
|---|---|---|
| Windows | `nsis/QuotaPulse_<versión>_x64-setup.exe`, `msi/QuotaPulse_<versión>_x64_en-US.msi` | `npm run tauri build` |
| macOS | `macos/QuotaPulse.app`, `dmg/QuotaPulse_<versión>_universal.dmg` | `npm run tauri build -- --target universal-apple-darwin --bundles app,dmg` |
| Linux | `appimage/QuotaPulse_<versión>_amd64.AppImage`, `deb/QuotaPulse_<versión>_amd64.deb` | `npm run tauri build -- --bundles appimage,deb` |

En macOS la app se firma ad hoc (`"signingIdentity": "-"`) y no se notariza: no hace falta
una cuenta de desarrollador de Apple. Para notarizar habría que añadir los secrets `APPLE_*`
que documenta Tauri.

`npm run tauri` pasa por `scripts/tauri.mjs`, que llama a la CLI de Tauri y, en Windows, tras un
`build` correcto, firma también el `QuotaPulse.exe` suelto (Tauri solo firma la copia que
mete en los instaladores).

## Firma de código

`bundle.windows.signCommand` ejecuta `src-tauri/scripts/sign-windows.ps1` sobre cada artefacto.
Elige el certificado en este orden:

1. `QP_SIGN_PFX` (+ `QP_SIGN_PFX_PASSWORD`): certificado `.pfx` de firma de código.
2. `QP_SIGN_THUMBPRINT`: certificado ya instalado en `Cert:\CurrentUser\My`.
3. Si no hay ninguno, crea (una vez) y reutiliza un certificado autofirmado
   `CN=QuotaPulse Local Code Signing` en `Cert:\CurrentUser\My`.

No contacta servidores de sellado de tiempo ni modifica los almacenes de confianza del equipo.

> Un certificado autofirmado solo demuestra que el binario no se modificó tras compilarlo.
> Smart App Control y SmartScreen confían en editores cuyo certificado encadena al Microsoft
> Trusted Root Program (certificado OV/EV o Azure Trusted Signing). Para distribuir a otros
> equipos usa la opción 1 o 2.

## Releases y actualizaciones automáticas

QuotaPulse se actualiza solo desde GitHub Releases con el updater de Tauri. Cada
actualización va firmada y la app la verifica con la clave pública incluida en
`src-tauri/tauri.conf.json` antes de instalarla.

**Configuración inicial (una vez):**

1. La clave privada del updater está en `%USERPROFILE%\.tauri\quotapulse-updater.key` y su
   contraseña en `%USERPROFILE%\.tauri\quotapulse-updater.password` (ambas fuera del
   repositorio). Haz copia de seguridad de las dos: si se pierden, las instalaciones
   existentes no podrán recibir más actualizaciones.
2. Ambas están como secrets del repositorio (GitHub → Settings → Secrets and variables →
   Actions): `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. La
   contraseña no puede estar vacía: en Windows una variable vacía no existe y la clave
   no se puede descifrar.

**Publicar una versión:**

1. Sube la versión (por ejemplo `1.0.1`) en `src-tauri/tauri.conf.json`, `package.json`,
   `package-lock.json` (las dos primeras apariciones), `src-tauri/Cargo.toml` y la entrada
   `quotapulse` de `src-tauri/Cargo.lock`, y haz commit. La interfaz lee la versión
   instalada en tiempo de ejecución, así que no hay ninguna versión escrita a mano en
   `src/`. CI y Release fallan si `package.json`, `tauri.conf.json` y `Cargo.toml` no
   coinciden.
2. Crea y sube la etiqueta correspondiente:

```bash
git tag v1.0.1
```

```bash
git push origin v1.0.1
```

`.github/workflows/release.yml` crea un borrador de Release, compila en paralelo en
Windows (`.exe` + `.msi`), macOS (`.app` + `.dmg` universales) y Linux (`.AppImage` +
`.deb`), firma los paquetes de actualización de cada plataforma y los une en un solo
`latest.json`; al terminar los tres, publica la Release. Así el actualizador nunca ve
un `latest.json` a medias. Las copias instaladas la
detectan al arrancar (si "update checks" está activo en Ajustes) o desde
*About → Check for updates*. GitHub Actions es gratuito en repositorios públicos.

Las builds locales no generan artefactos de actualización (no necesitan la clave);
el workflow los activa con `--config src-tauri/tauri.release.conf.json`.

## Pruebas

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

La lógica que depende del sistema (rutas de las CLI, nombres de proceso, comandos de
terminal para el login, posición de la cápsula) está en funciones puras de
`src-tauri/src/platform.rs` y `capsule.rs` que reciben el sistema como parámetro, así
que estos tests comprueban Windows, macOS y Linux desde cualquier equipo. El workflow
`ci.yml` ejecuta además `cargo test` y `cargo clippy` en `windows-latest`, `macos-latest`
y `ubuntu-22.04` en cada push y PR, para compilar de verdad el código de cada plataforma.

## Rutas útiles

| Qué | Windows | macOS | Linux |
|---|---|---|---|
| Datos de la app (SQLite, logs) | `%APPDATA%\com.quotapulse.desktop` | `~/Library/Application Support/com.quotapulse.desktop` | `~/.local/share/com.quotapulse.desktop` |
| Ajustes de la cápsula (`capsule.json`) | `%APPDATA%\com.quotapulse.desktop` | `~/Library/Application Support/com.quotapulse.desktop` | `~/.config/com.quotapulse.desktop` |
| Log de LiteLLM | `%LOCALAPPDATA%\QuotaPulse\litellm.log` | — | — |
| Entorno de LiteLLM | `.venv` junto a `start-litellm.bat` | — | — |
