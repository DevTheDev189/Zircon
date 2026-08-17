@echo off
rem ===========================================================================
rem  Zircon build script
rem  Compiles the server wrapper exe and the launcher bundles (exe + msi).
rem
rem  Commands mirror the manual release build:
rem    - cargo build --release -p zircon-server
rem    - repackage dist-run/zircon-server-distribution.zip
rem    - npx @tauri-apps/cli build  (in crates/zircon-launcher)
rem ===========================================================================
setlocal
cd /d "%~dp0"

echo.
echo === [1/3] Building server release exe ===
cargo build --release -p zircon-server
if errorlevel 1 (
    echo FAILED: server release build
    exit /b 1
)

echo.
echo === [2/3] Packaging server distribution zip ===
copy /Y "target\release\zircon-server.exe" "dist-run\zircon-server\zircon-server.exe"
if errorlevel 1 (
    echo FAILED: could not copy server exe to dist-run
    exit /b 1
)
rem Keep the empty server-data folder in the zip (zip tools drop empty dirs).
if not exist "dist-run\zircon-server\server-data\.keep" mkdir "dist-run\zircon-server\server-data\.keep"
> "dist-run\zircon-server\server-data\.keep\readme.txt" echo placeholder
if exist "dist-run\zircon-server-distribution.zip" del /Q "dist-run\zircon-server-distribution.zip"
powershell -NoProfile -Command "Compress-Archive -Path dist-run/zircon-server -DestinationPath dist-run/zircon-server-distribution.zip"
if errorlevel 1 (
    echo FAILED: could not create dist-run/zircon-server-distribution.zip
    exit /b 1
)

echo.
echo === [3/3] Building launcher bundles (exe + msi) ===
cd /d "%~dp0crates\zircon-launcher"
call npx --yes @tauri-apps/cli build
if errorlevel 1 (
    echo FAILED: launcher bundle build
    exit /b 1
)
cd /d "%~dp0"

echo.
echo ===========================================================================
echo Done. Artifacts:
echo   target\release\zircon-launcher.exe
echo   target\release\bundle\msi\Zircon_0.1.0_x64_en-US.msi
echo   target\release\bundle\nsis\Zircon_0.1.0_x64-setup.exe
echo   dist-run\zircon-server\zircon-server.exe
echo   dist-run\zircon-server-distribution.zip
echo ===========================================================================
endlocal
