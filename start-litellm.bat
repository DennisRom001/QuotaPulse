@echo off
rem ---------------------------------------------------------------------------
rem QuotaPulse - local LiteLLM Proxy launcher
rem
rem  * Uses an isolated virtual environment ".venv" next to this script (the
rem    project root in development, the QuotaPulse install folder once
rem    installed). Global Python packages are never touched: every install and
rem    the server itself run through .venv\Scripts\python.exe.
rem  * Starts the proxy on 127.0.0.1:4000 in the background (loopback only).
rem  * Output goes to %LOCALAPPDATA%\QuotaPulse\litellm.log
rem
rem QuotaPulse runs this automatically (hidden) when port 4000 is closed.
rem Set QP_DISABLE_LITELLM=1 to stop QuotaPulse from doing so.
rem ---------------------------------------------------------------------------
setlocal
cd /d "%~dp0"

rem Version pinned on purpose: LiteLLM releases 1.82.7/1.82.8 were compromised
rem on PyPI in March 2026. Bump only to a release you have reviewed.
set "LITELLM_VERSION=1.105.7"

rem Master key for the local proxy (required by LiteLLM; avoids
rem UnsafeMasterKeyError). An existing LITELLM_MASTER_KEY from the environment
rem wins; otherwise a development-only default is used. The proxy only listens
rem on 127.0.0.1, but set your own key for anything beyond local testing.
if not defined LITELLM_MASTER_KEY set "LITELLM_MASTER_KEY=sk-quota-pulse-local"

rem Keep LiteLLM offline-friendly: use the bundled model price map instead of
rem downloading it at startup, and no telemetry.
set "LITELLM_LOCAL_MODEL_COST_MAP=True"
set "LITELLM_TELEMETRY=False"

set "VENV=%~dp0.venv"
set "PY=%VENV%\Scripts\python.exe"
set "LOG_DIR=%LOCALAPPDATA%\QuotaPulse"
set "LOG=%LOG_DIR%\litellm.log"
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"

rem Already running? Nothing to do.
netstat -ano -p TCP | findstr /R /C:"127\.0\.0\.1:4000 .*LISTENING" >nul 2>&1
if not errorlevel 1 (
  echo [%date% %time%] LiteLLM already listening on 127.0.0.1:4000>>"%LOG%"
  exit /b 0
)

rem 1. Isolated environment: create .venv with the system Python only once.
if not exist "%PY%" (
  where python >nul 2>&1
  if errorlevel 1 (
    echo [%date% %time%] ERROR: Python 3.10-3.14 not found on PATH>>"%LOG%"
    exit /b 1
  )
  echo [%date% %time%] creating virtual environment %VENV%>>"%LOG%"
  python -m venv "%VENV%" >>"%LOG%" 2>&1 || exit /b 1
)

rem 2. Pinned LiteLLM + server deps, installed only through the .venv Python.
"%PY%" -c "import importlib.metadata as m, sys; sys.exit(m.version('litellm') != '%LITELLM_VERSION%')" >nul 2>&1
if errorlevel 1 (
  echo [%date% %time%] installing litellm[proxy]==%LITELLM_VERSION% into .venv>>"%LOG%"
  "%PY%" -m pip install --disable-pip-version-check --upgrade "litellm[proxy]==%LITELLM_VERSION%" uvicorn fastapi >>"%LOG%" 2>&1 || exit /b 1
)

rem 3. Start the server in the background with the isolated interpreter.
echo [%date% %time%] starting LiteLLM %LITELLM_VERSION% from .venv on 127.0.0.1:4000>>"%LOG%"
start "" /b "%PY%" -c "import uvicorn; from litellm.proxy.proxy_server import app; uvicorn.run(app, host='127.0.0.1', port=4000)" >>"%LOG%" 2>&1
exit /b 0
