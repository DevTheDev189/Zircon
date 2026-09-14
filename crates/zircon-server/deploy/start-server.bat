@echo off
setlocal
cd /d "%~dp0"
title Zircon Server Manager

echo ===========================================================================
echo   Zircon Server Manager Daemon
echo ===========================================================================
echo   Web Dashboard:       http://localhost:25564
echo   Minecraft Game Port: 25565
echo.
echo   * On first startup, your initial Admin password will be printed below.
echo   * Press Ctrl+C at any time to gracefully stop the server.
echo ===========================================================================
echo.

if not exist "%~dp0zircon-server.exe" (
    echo [ERROR] zircon-server.exe not found in this directory.
    pause
    exit /b 1
)

"%~dp0zircon-server.exe" %*
if errorlevel 1 (
    echo.
    echo [NOTICE] Zircon Server stopped or exited with error code (%errorlevel%).
    pause
)
endlocal
