@echo off
rem ===========================================================================
rem  release.bat
rem  One-Click Multi-Platform Release & Deployment Script
rem  Usage:
rem    release.bat 0.4.8
rem ===========================================================================
setlocal enabledelayedexpansion
cd /d "%~dp0"

set VERSION=%1
if "%VERSION%"=="" (
    set /p VERSION="Enter release version (e.g. 0.4.8): "
)
if "%VERSION%"=="" (
    echo [ERROR] Version cannot be empty.
    pause
    exit /b 1
)
if "%VERSION:~0,1%"=="v" set VERSION=%VERSION:~1%

echo.
echo ===========================================================================
echo   Publishing Zircon Release v%VERSION%
echo ===========================================================================
echo.

echo [1/4] Syncing version %VERSION% across project configs and website...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\sync-version.ps1" "%VERSION%"
if errorlevel 1 (
    echo [ERROR] Failed to sync version.
    pause
    exit /b 1
)

echo.
echo [2/4] Committing version bump...
git add crates/zircon-launcher/tauri.conf.json crates/zircon-launcher/ui/package.json crates/zircon-launcher/Cargo.toml crates/zircon-server/Cargo.toml crates/zircon-core/Cargo.toml website/ scripts/sync-version.ps1
git commit -m "Release v%VERSION%"

echo.
echo [3/4] Creating tag v%VERSION%...
git tag -f "v%VERSION%"

echo.
echo [4/4] Pushing to GitHub (main + tags)...
git push origin main --tags
if errorlevel 1 (
    echo [ERROR] Git push failed.
    pause
    exit /b 1
)

echo.
echo ===========================================================================
echo   Release v%VERSION% Dispatched Successfully!
echo ===========================================================================
echo GitHub Actions is now compiling for Windows, macOS, and Linux in parallel.
echo Once builds complete, GitHub Actions will automatically:
echo   1. Create the GitHub Release with all multi-platform binaries.
echo   2. Deploy the website to Cloudflare Pages (zirconmc.net).
echo   3. Upload all installers and updater manifests to Cloudflare R2 (downloads.zirconmc.net).
echo.
echo Live progress:
echo   https://github.com/DevTheDev189/Zircon/actions
echo ===========================================================================
echo.
pause
endlocal
