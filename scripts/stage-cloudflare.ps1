# ===========================================================================
# stage-cloudflare.ps1
# Multi-platform staging pipeline:
#   1. Ingests GitHub Actions build artifacts (Windows, macOS, Linux).
#   2. Generates server & launcher auto-updater manifests (latest.json).
#   3. Packages R2 download tree into 'cloudflare-r2/' (for downloads.zirconmc.net).
#   4. Packages static website into 'cloudflare-pages/' (for zirconmc.net).
# ===========================================================================
param(
    [string]$Version = "0.4.7",
    [string]$Domain = "https://downloads.zirconmc.net",
    [string]$ArtifactsDir = "download-artifacts",
    [string]$PagesUploadDir = "cloudflare-pages",
    [string]$R2UploadDir = "cloudflare-r2"
)

$ErrorActionPreference = "Stop"
$rootDir = Split-Path -Path $PSScriptRoot -Parent
Set-Location -Path $rootDir

Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host " Zircon Multi-Platform Release Staging (v$Version)" -ForegroundColor Cyan
Write-Host " Website -> Cloudflare Pages (zirconmc.net)" -ForegroundColor Cyan
Write-Host " Binaries -> Cloudflare R2   (downloads.zirconmc.net)" -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan

# ---------------------------------------------------------------------------
# 1. Pull Latest Artifacts from GitHub Actions (if `gh` CLI available)
# ---------------------------------------------------------------------------
$artifactsPath = Join-Path $rootDir $ArtifactsDir
$hasGh = (Get-Command "gh" -ErrorAction SilentlyContinue) -ne $null

if ($hasGh) {
    Write-Host "`n[1/5] GitHub CLI detected. Fetching latest build artifacts..." -ForegroundColor Green
    try {
        if (-not (Test-Path $artifactsPath)) { New-Item -ItemType Directory -Path $artifactsPath -Force | Out-Null }
        & gh run download --repo DevTheDev189/Zircon --dir $artifactsPath
        Write-Host "Successfully downloaded artifacts from GitHub Actions into $ArtifactsDir." -ForegroundColor Green
    } catch {
        Write-Host "Warning: Could not automatically download via gh: $_" -ForegroundColor Yellow
        Write-Host "Falling back to existing files in $ArtifactsDir." -ForegroundColor Yellow
    }
} else {
    Write-Host "`n[1/5] Checking for artifacts in '$ArtifactsDir'..." -ForegroundColor Yellow
    if (-not (Test-Path $artifactsPath)) {
        New-Item -ItemType Directory -Path $artifactsPath -Force | Out-Null
        Write-Host "Created '$ArtifactsDir'. If you downloaded artifact zips from GitHub Actions," -ForegroundColor Cyan
        Write-Host "extract them into: $artifactsPath" -ForegroundColor Cyan
    }
}

# ---------------------------------------------------------------------------
# 2. Reset Staging Directories
# ---------------------------------------------------------------------------
Write-Host "`n[2/5] Initializing clean staging directories..." -ForegroundColor Green
$r2Path = Join-Path $rootDir $R2UploadDir
$pagesPath = Join-Path $rootDir $PagesUploadDir

if (Test-Path $r2Path) { Remove-Item -Recurse -Force $r2Path }
if (Test-Path $pagesPath) { Remove-Item -Recurse -Force $pagesPath }

$serverStageDir = Join-Path $r2Path "updates/server/v$Version"
$launcherStageDir = Join-Path $r2Path "updates/launcher"
New-Item -ItemType Directory -Path $serverStageDir -Force | Out-Null
New-Item -ItemType Directory -Path $launcherStageDir -Force | Out-Null
New-Item -ItemType Directory -Path $pagesPath -Force | Out-Null

# ---------------------------------------------------------------------------
# 3. Process & Stage Multi-Platform Server Binaries (R2)
# ---------------------------------------------------------------------------
Write-Host "`n[3/5] Staging Server packages & generating server manifest..." -ForegroundColor Green

# Locate Windows server zip (from artifacts or local dist-run)
$serverWinZip = Get-ChildItem -Path @($artifactsPath, "dist-run") -Filter "*server-windows*.zip" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
$serverLinuxTar = Get-ChildItem -Path @($artifactsPath, "dist-run") -Filter "*server-linux*.tar.gz" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
$serverLinuxZip = Get-ChildItem -Path @($artifactsPath, "dist-run") -Filter "*server-linux*.zip" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1

$serverPlatforms = @{}

if ($serverWinZip) {
    $destWin = Join-Path $serverStageDir "zircon-server-windows-x86_64.zip"
    Copy-Item $serverWinZip.FullName -Destination $destWin -Force
    $winHash = (Get-FileHash -Path $destWin -Algorithm SHA256).Hash
    $serverPlatforms["windows-x86_64"] = @{
        url = "$Domain/updates/server/v$Version/zircon-server-windows-x86_64.zip"
        sha256 = $winHash
        binName = "zircon-server.exe"
    }
    Write-Host "  -> Windows Server staged (SHA256: $winHash)" -ForegroundColor Gray
}

if ($serverLinuxTar) {
    $destLinuxTar = Join-Path $serverStageDir "zircon-server-linux-x86_64.tar.gz"
    Copy-Item $serverLinuxTar.FullName -Destination $destLinuxTar -Force
    Write-Host "  -> Linux Server tar.gz staged" -ForegroundColor Gray
}

if ($serverLinuxZip) {
    $destLinuxZip = Join-Path $serverStageDir "zircon-server-linux-x86_64.zip"
    Copy-Item $serverLinuxZip.FullName -Destination $destLinuxZip -Force
    $linuxHash = (Get-FileHash -Path $destLinuxZip -Algorithm SHA256).Hash
    $serverPlatforms["linux-x86_64"] = @{
        url = "$Domain/updates/server/v$Version/zircon-server-linux-x86_64.zip"
        sha256 = $linuxHash
        binName = "zircon-server"
    }
    Write-Host "  -> Linux Server zip staged (for auto-updater) (SHA256: $linuxHash)" -ForegroundColor Gray
} elseif ($serverLinuxTar) {
    $linuxHash = (Get-FileHash -Path $destLinuxTar -Algorithm SHA256).Hash
    $serverPlatforms["linux-x86_64"] = @{
        url = "$Domain/updates/server/v$Version/zircon-server-linux-x86_64.tar.gz"
        sha256 = $linuxHash
        binName = "zircon-server"
    }
}

$serverManifest = @{
    version = $Version
    releaseDate = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
    notes = "Zircon Server Release v$Version"
    platforms = $serverPlatforms
}
$serverLatestJson = ConvertTo-Json $serverManifest -Depth 4
$serverLatestFile = Join-Path $r2Path "updates/server/latest.json"
[System.IO.File]::WriteAllText($serverLatestFile, $serverLatestJson, [System.Text.UTF8Encoding]::new($false))
Write-Host "  -> updates/server/latest.json generated." -ForegroundColor Gray

# ---------------------------------------------------------------------------
# 4. Process & Stage Multi-Platform Launcher Binaries & Updater Manifest (R2)
# ---------------------------------------------------------------------------
Write-Host "`n[4/5] Staging Launcher bundles (Windows, macOS, Linux)..." -ForegroundColor Green

# Collect all launcher binary files (.exe, .msi, .dmg, .AppImage, .deb, .rpm, .tar.gz, .sig)
$launcherFiles = Get-ChildItem -Path @($artifactsPath, "target/release/bundle") -Include "*.exe", "*.msi", "*.dmg", "*.AppImage", "*.deb", "*.rpm", "*.tar.gz", "*.sig", "*.zip" -Recurse -ErrorAction SilentlyContinue

$launcherPlatforms = @{}
$pubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

foreach ($file in $launcherFiles) {
    if ($file.Name -like "*server*") { continue }
    if ($file.Name -like "*control.tar.gz*" -or $file.Name -like "*data.tar.gz*") { continue }
    # Only stage artifacts matching the target version or unversioned bundles
    if ($file.Name -match '\d+\.\d+\.\d+' -and $file.Name -notlike "*$Version*") {
        Write-Host "  Skipping older version artifact: $($file.Name)" -ForegroundColor DarkGray
        continue
    }
    Copy-Item $file.FullName -Destination $launcherStageDir -Force
    Write-Host "  Staged: $($file.Name)" -ForegroundColor Gray
}

$winSetup = Get-ChildItem -Path $launcherStageDir -Filter "*setup.exe" | Select-Object -First 1
$winSig = Get-ChildItem -Path $launcherStageDir -Filter "*setup.exe.sig" | Select-Object -First 1
if ($winSetup) {
    $sigContent = if ($winSig) { [System.IO.File]::ReadAllText($winSig.FullName).Trim() } else { "" }
    $launcherPlatforms["windows-x86_64"] = @{
        signature = $sigContent
        url = "$Domain/updates/launcher/$($winSetup.Name)"
    }
}

$linuxAppImage = Get-ChildItem -Path $launcherStageDir -Filter "*.AppImage" | Select-Object -First 1
$linuxSig = Get-ChildItem -Path $launcherStageDir -Filter "*.AppImage.sig" | Select-Object -First 1
if ($linuxAppImage) {
    $sigContent = if ($linuxSig) { [System.IO.File]::ReadAllText($linuxSig.FullName).Trim() } else { "" }
    $launcherPlatforms["linux-x86_64"] = @{
        signature = $sigContent
        url = "$Domain/updates/launcher/$($linuxAppImage.Name)"
    }
}

$macDmg = Get-ChildItem -Path $launcherStageDir -Filter "*.dmg" | Select-Object -First 1
$macSig = Get-ChildItem -Path $launcherStageDir -Filter "*.dmg.sig" | Select-Object -First 1
if ($macDmg) {
    $sigContent = if ($macSig) { [System.IO.File]::ReadAllText($macSig.FullName).Trim() } else { "" }
    $launcherPlatforms["darwin-aarch64"] = @{
        signature = $sigContent
        url = "$Domain/updates/launcher/$($macDmg.Name)"
    }
    $launcherPlatforms["darwin-x86_64"] = @{
        signature = $sigContent
        url = "$Domain/updates/launcher/$($macDmg.Name)"
    }
}

$launcherManifest = @{
    version = $Version
    notes = "Zircon Launcher Release v$Version"
    pub_date = $pubDate
    platforms = $launcherPlatforms
}
$launcherLatestJson = ConvertTo-Json $launcherManifest -Depth 4
$launcherLatestFile = Join-Path $launcherStageDir "latest.json"
[System.IO.File]::WriteAllText($launcherLatestFile, $launcherLatestJson, [System.Text.UTF8Encoding]::new($false))
Write-Host "  -> updates/launcher/latest.json generated." -ForegroundColor Gray

# ---------------------------------------------------------------------------
# 5. Copy Website & Stamp Release Version (Pages)
# ---------------------------------------------------------------------------
Write-Host "`n[5/5] Copying website to '$PagesUploadDir' & stamping release version v$Version..." -ForegroundColor Green
$websiteDir = Join-Path $rootDir "website"

robocopy $websiteDir $pagesPath /E /XD ".idea" /NFL /NDL /NJH /NJS /NP | Out-Null

$stagedFiles = Get-ChildItem -Path "$pagesPath\*.html", "$pagesPath\assets\js\*.js"
foreach ($f in $stagedFiles) {
    $content = [System.IO.File]::ReadAllText($f.FullName)
    $content = [regex]::Replace($content, 'v\d+\.\d+\.\d+', "v$Version")
    $content = [regex]::Replace($content, 'Zircon_\d+\.\d+\.\d+_', "Zircon_${Version}_")
    $content = [regex]::Replace($content, 'zircon_\d+\.\d+\.\d+_', "zircon_${Version}_")
    $content = [regex]::Replace($content, 'zircon-\d+\.\d+\.\d+-', "zircon-${Version}-")
    [System.IO.File]::WriteAllText($f.FullName, $content, [System.Text.UTF8Encoding]::new($false))
}
Write-Host "  -> Website stamped with v$Version and downloads.zirconmc.net URLs." -ForegroundColor Gray

Write-Host "`n=================================================================" -ForegroundColor Cyan
Write-Host " Staging Complete!" -ForegroundColor Green
Write-Host " 1. Website (Cloudflare Pages): $pagesPath" -ForegroundColor White
Write-Host " 2. Binaries (Cloudflare R2):   $r2Path" -ForegroundColor White
Write-Host "=================================================================" -ForegroundColor Cyan
