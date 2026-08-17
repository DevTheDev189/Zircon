@echo off
rem Zircon server wrapper launcher (fresh-install / distribution build).
rem
rem Sets the data directory explicitly (via MC_MANAGER_DATA_DIR) so the server
rem always reads/writes ./server-data here, regardless of where the exe or
rem this bat is invoked from. On the first run this folder is created empty;
rem the wrapper seeds it with config, a fresh admin password (printed here),
rem and its jav/ JWT secret.
setlocal
cd /d "%~dp0"
if not exist "server-data" mkdir "server-data"
set "MC_MANAGER_DATA_DIR=%~dp0server-data"
echo Starting Zircon server wrapper - data dir: %MC_MANAGER_DATA_DIR%
echo Admin UI will be at http://localhost:25564 (or http://localhost:25565)
echo On first run the admin password is printed to this console.
echo Press Ctrl+C to stop.
zircon-server.exe
