@echo off
rem ---------------------------------------------------------------------------
rem QuotaPulse - local LiteLLM Proxy launcher
rem
rem  * Uses an isolated virtual environment ".venv" next to this script (the
rem    project root in development, the QuotaPulse install folder once
rem    installed). Global Python packages are never touched: every install and
rem    the server itself run through .venv\Scripts\python.exe.
rem  * Installs ONLY the hash-pinned dependency set in litellm-requirements.txt
rem    (pip --require-hashes --no-deps): no version floats, nothing unreviewed.
rem  * Starts the proxy on 127.0.0.1:4000 in the background (loopback only).
rem  * Output goes to %LOCALAPPDATA%\QuotaPulse\litellm.log
rem
rem QuotaPulse runs this automatically (hidden) when port 4000 is closed and
rem passes a random per-install LITELLM_MASTER_KEY plus a minimal environment
rem (no provider API keys). Set QP_DISABLE_LITELLM=1 to stop QuotaPulse from
rem doing so.
rem ---------------------------------------------------------------------------
setlocal
cd /d "%~dp0"

rem Version pinned on purpose: LiteLLM releases 1.82.7/1.82.8 were compromised
rem on PyPI in March 2026. The whole dependency closure is pinned with hashes
rem in litellm-requirements.txt; regenerate it only from a reviewed release.
set "LITELLM_VERSION=1.105.7"
set "REQS=%~dp0litellm-requirements.txt"

set "SYS32=%SystemRoot%\System32"
set "VENV=%~dp0.venv"
set "PY=%VENV%\Scripts\python.exe"
set "STAMP=%VENV%\quotapulse-requirements.txt"
set "LOG_DIR=%LOCALAPPDATA%\QuotaPulse"
set "LOG=%LOG_DIR%\litellm.log"
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"

rem Master key: required, and never a built-in default. QuotaPulse provides a
rem random per-install key; set LITELLM_MASTER_KEY yourself when running this
rem script by hand.
if not defined LITELLM_MASTER_KEY (
  echo [%date% %time%] ERROR: LITELLM_MASTER_KEY is not set; refusing to start with no key>>"%LOG%"
  exit /b 1
)

rem Responses are not readable from web pages: only the proxy's own origin.
set "LITELLM_CORS_ORIGINS=http://127.0.0.1:4000"
rem Offline-friendly: bundled model price map, no telemetry.
set "LITELLM_LOCAL_MODEL_COST_MAP=True"
set "LITELLM_TELEMETRY=False"

rem Already running? Nothing to do.
"%SYS32%\netstat.exe" -ano -p TCP | "%SYS32%\findstr.exe" /R /C:"127\.0\.0\.1:4000 .*LISTENING" >nul 2>&1
if not errorlevel 1 (
  echo [%date% %time%] LiteLLM already listening on 127.0.0.1:4000>>"%LOG%"
  exit /b 0
)

if not exist "%REQS%" (
  echo [%date% %time%] ERROR: litellm-requirements.txt is missing next to the script>>"%LOG%"
  exit /b 1
)

rem 1. Isolated environment: create .venv with the system Python only once.
if not exist "%PY%" (
  "%SYS32%\where.exe" python >nul 2>&1
  if errorlevel 1 (
    echo [%date% %time%] ERROR: Python 3.14 not found on PATH>>"%LOG%"
    exit /b 1
  )
  echo [%date% %time%] creating virtual environment .venv>>"%LOG%"
  python -I -m venv "%VENV%" >>"%LOG%" 2>&1 || exit /b 1
)

rem 2. Install the hash-pinned set whenever it differs from what .venv has.
"%SYS32%\fc.exe" /b "%REQS%" "%STAMP%" >nul 2>&1
if errorlevel 1 (
  echo [%date% %time%] installing hash-pinned litellm[proxy]==%LITELLM_VERSION% into .venv>>"%LOG%"
  "%PY%" -I -m pip install --disable-pip-version-check --require-hashes --no-deps -r "%REQS%" >>"%LOG%" 2>&1 || exit /b 1
  copy /y "%REQS%" "%STAMP%" >nul
)

rem 3. Start the server in the background with the isolated interpreter.
rem    -I keeps the current directory and user site-packages off sys.path.
echo [%date% %time%] starting LiteLLM %LITELLM_VERSION% from .venv on 127.0.0.1:4000>>"%LOG%"
start "" /b "%PY%" -I -c "import uvicorn; from litellm.proxy.proxy_server import app; uvicorn.run(app, host='127.0.0.1', port=4000)" >>"%LOG%" 2>&1
exit /b 0
