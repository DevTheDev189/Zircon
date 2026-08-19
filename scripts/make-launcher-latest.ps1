# Generates the Tauri updater manifest (bundle/updater/latest.json) for a
# signed Windows build. Local `tauri build` creates the installer + .sig files
# but NOT latest.json (that is normally produced by the tauri-action GitHub
# Action), so this script fills the gap for self-hosted R2 releases.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\make-launcher-latest.ps1
#
# The generated manifest points windows-x86_64 at the NSIS installer; upload
# it (plus the installer + .sig) to /updates/launcher/ on the R2 bucket.

param(
    [string]$Version = "0.1.0",
    [string]$Domain = "https://zirconmc.net",
    [string]$BundleDir = "target/release/bundle"
)

$nsisDir = Join-Path $BundleDir "nsis"
$installer = Join-Path $nsisDir "Zircon_${Version}_x64-setup.exe"
$sigFile = "$installer.sig"
$outDir = Join-Path $BundleDir "updater"
$outFile = Join-Path $outDir "latest.json"

if (-not (Test-Path $sigFile)) {
    Write-Error "Signature not found: $sigFile (was the launcher built with the signing key?)"
    exit 1
}

New-Item -ItemType Directory -Force -Path $outDir | Out-Null

# Reads the signature as a plain .NET string (Get-Content -Raw can return an
# object in some PowerShell versions, which then serializes into the JSON as
# a nested object full of PSPath/PSProvider noise instead of a string).
$sig = [System.IO.File]::ReadAllText($sigFile)
$artifactUrl = "$Domain/updates/launcher/Zircon_${Version}_x64-setup.exe"

$manifest = [ordered]@{
    version  = $Version
    notes    = "Zircon Launcher Release v$Version"
    pub_date = (Get-Date).ToString("o")
    platforms = @{
        "windows-x86_64" = @{
            url       = $artifactUrl
            signature = $sig
        }
    }
}

$manifest | ConvertTo-Json -Depth 5 | Set-Content -NoNewline -Path $outFile
Write-Host "Generated $outFile -> $artifactUrl"
