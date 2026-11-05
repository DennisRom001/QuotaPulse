# Compilar QuotaPulse

## Requisitos

- Node.js 20+ y npm
- Rust estable (`rustup`) con el target MSVC
- Visual Studio Build Tools con la carga de trabajo **Desarrollo para el escritorio con C++**
- Windows 10/11

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

Genera en `src-tauri/target/release/`:

- `QuotaPulse.exe`
- `bundle/nsis/QuotaPulse_<versión>_x64-setup.exe`
- `bundle/msi/QuotaPulse_<versión>_x64_en-US.msi`

`npm run tauri` pasa por `scripts/tauri.mjs`, que llama a la CLI de Tauri y, tras un
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

1. Sube la versión en `src-tauri/tauri.conf.json`, `package.json`, `src-tauri/Cargo.toml`
   y `src/constants/appMeta.ts` (por ejemplo `1.0.1`) y haz commit.
2. Crea y sube la etiqueta correspondiente:

```bash
git tag v1.0.1
```

```bash
git push origin v1.0.1
```

`.github/workflows/release.yml` compila en Windows, firma los paquetes de
actualización, genera `latest.json` y publica la Release. Las copias instaladas la
detectan al arrancar (si "update checks" está activo en Ajustes) o desde
*About → Check for updates*. GitHub Actions es gratuito en repositorios públicos.

Las builds locales no generan artefactos de actualización (no necesitan la clave);
el workflow los activa con `--config src-tauri/tauri.release.conf.json`.

## Pruebas

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## Rutas útiles

| Qué | Dónde |
|---|---|
| Datos de la app (SQLite, ajustes) | `%APPDATA%\com.quotapulse.desktop` |
| Logs | `%APPDATA%\QuotaPulse\logs` |
| Log de LiteLLM | `%LOCALAPPDATA%\QuotaPulse\litellm.log` |
| Entorno de LiteLLM | `.venv` junto a `start-litellm.bat` |
