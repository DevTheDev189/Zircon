@echo off
rem ===========================================================================
rem  Zircon build & release packaging script
rem  Compiles the server wrapper exe and the launcher bundles (exe + msi),
rem  and emits the updater manifests/artifacts for Cloudflare R2.
rem
rem  Commands mirror the manual release build:
rem    - cargo build --release -p zircon-server
rem    - repackage dist-run/zircon-server-windows-x86_64.zip + server-latest.json
rem    - npx @tauri-apps/cli build  (in crates/zircon-launcher, emits the
rem      signed launcher bundles + updater/latest.json when
rem      TAURI_SIGNING_PRIVATE_KEY is set and tauri.conf.json's
rem      plugins.updater.active is true)
rem ===========================================================================
setlocal enabledelayedexpansion
cd /d "%~dp0"

set VERSION=0.1.0
set DOMAIN=https://zirconmc.net

echo.
echo === [1/3] Building server release exe ===
cargo build --release -p zircon-server
if errorlevel 1 (
    echo FAILED: server release build
    exit /b 1
)

echo.
echo === [2/3] Packaging server distribution zip ^& updater metadata ===
if not exist "dist-run\zircon-server" mkdir "dist-run\zircon-server"
copy /Y "target\release\zircon-server.exe" "dist-run\zircon-server\zircon-server.exe"
if errorlevel 1 (
    echo FAILED: could not copy server exe to dist-run
    exit /b 1
)
rem Keep the empty server-data folder in the zip (zip tools drop empty dirs).
if not exist "dist-run\zircon-server\server-data\.keep" mkdir "dist-run\zircon-server\server-data\.keep"
> "dist-run\zircon-server\server-data\.keep\readme.txt" echo placeholder

if exist "dist-run\zircon-server-windows-x86_64.zip" del /Q "dist-run\zircon-server-windows-x86_64.zip"
powershell -NoProfile -Command "Compress-Archive -Path dist-run/zircon-server/* -DestinationPath dist-run/zircon-server-windows-x86_64.zip -Force"
if errorlevel 1 (
    echo FAILED: could not create server zip
    exit /b 1
)

for /f %%i in ('certutil -hashfile dist-run\zircon-server-windows-x86_64.zip SHA256 ^| findstr /v "hash"') do set HASH=%%i
set HASH=%HASH: =%

echo Generating dist-run\server-latest.json (SHA256: %HASH%)...
(
  echo {
  echo   "version": "%VERSION%",
  echo   "releaseDate": "%DATE% %TIME%",
  echo   "notes": "Zircon Server Release v%VERSION%",
  echo   "platforms": {
  echo     "windows-x86_64": {
  echo       "url": "%DOMAIN%/updates/server/v%VERSION%/zircon-server-windows-x86_64.zip",
  echo       "sha256": "%HASH%",
  echo       "binName": "zircon-server.exe"
  echo     }
  echo   }
  echo }
) > dist-run\server-latest.json

echo.
echo === [3/3] Building launcher bundles with updater artifacts ===
rem The Tauri CLI signs the MSI/NSIS installers (emits .sig files alongside them
rem when bundle.createUpdaterArtifacts is true and TAURI_SIGNING_PRIVATE_KEY is
rem set) but does NOT generate an updater/latest.json manifest itself for
rem self-hosted feeds -- that auto-generation only happens in the GitHub Actions
rem tauri-action flow. Build one by hand from the NSIS installer + its .sig,
rem the same way the server manifest is built above.
cd /d "%~dp0crates\zircon-launcher"
call npx --yes @tauri-apps/cli build
if errorlevel 1 (
    echo FAILED: launcher bundle build
    exit /b 1
)
cd /d "%~dp0"

set NSIS_EXE=target\release\bundle\nsis\Zircon_%VERSION%_x64-setup.exe
if not exist "%NSIS_EXE%.sig" (
    echo FAILED: no signature found at %NSIS_EXE%.sig -- was TAURI_SIGNING_PRIVATE_KEY set?
    exit /b 1
)
set /p LAUNCHER_SIG=<"%NSIS_EXE%.sig"

for /f %%i in ('powershell -NoProfile -Command "(Get-Date).ToUniversalTime().ToString(\"yyyy-MM-ddTHH:mm:ssZ\")"') do set PUBDATE=%%i

echo Generating dist-run\launcher-latest.json...
(
  echo {
  echo   "version": "%VERSION%",
  echo   "notes": "Zircon Launcher Release v%VERSION%",
  echo   "pub_date": "%PUBDATE%",
  echo   "platforms": {
  echo     "windows-x86_64": {
  echo       "signature": "%LAUNCHER_SIG%",
  echo       "url": "%DOMAIN%/updates/launcher/Zircon_%VERSION%_x64-setup.exe"
  echo     }
  echo   }
  echo }
) > dist-run\launcher-latest.json

echo.
echo ===========================================================================
echo Build completed successfully!
echo.
echo Artifacts to upload to Cloudflare R2:
echo   - dist-run\server-latest.json -^> https://zirconmc.net/updates/server/latest.json
echo   - dist-run\zircon-server-windows-x86_64.zip -^> https://zirconmc.net/updates/server/v%VERSION%/zircon-server-windows-x86_64.zip
echo   - dist-run\launcher-latest.json -^> https://zirconmc.net/updates/launcher/latest.json
echo   - target\release\bundle\nsis\Zircon_%VERSION%_x64-setup.exe -^> https://zirconmc.net/updates/launcher/Zircon_%VERSION%_x64-setup.exe
echo   - (optional, unsigned-feed installers) target\release\bundle\msi\*.msi -^> https://zirconmc.net/updates/launcher/
echo ===========================================================================
endlocal
