# Syncs the release version from the build script into all project metadata files.
param(
    [Parameter(Mandatory=$true)]
    [string]$Version
)

$rootDir = Split-Path -Path $PSScriptRoot -Parent

$jsonFiles = @(
    (Join-Path $rootDir "crates/zircon-launcher/tauri.conf.json"),
    (Join-Path $rootDir "crates/zircon-launcher/ui/package.json")
)

$tomlFiles = @(
    (Join-Path $rootDir "crates/zircon-launcher/Cargo.toml"),
    (Join-Path $rootDir "crates/zircon-server/Cargo.toml"),
    (Join-Path $rootDir "crates/zircon-core/Cargo.toml")
)

foreach ($f in $jsonFiles) {
    if (Test-Path $f) {
        $lines = [System.IO.File]::ReadAllLines($f)
        for ($i = 0; $i -lt $lines.Count; $i++) {
            if ($lines[$i] -match '^\s*"version"\s*:') {
                $hasComma = $lines[$i].TrimEnd().EndsWith(',')
                $indent = $lines[$i].Substring(0, $lines[$i].IndexOf('"'))
                $comma = if ($hasComma) { ',' } else { '' }
                $lines[$i] = "${indent}`"version`": `"$Version`"$comma"
                break
            }
        }
        [System.IO.File]::WriteAllLines($f, $lines, [System.Text.UTF8Encoding]::new($false))
        Write-Host "Synced $f -> version $Version"
    }
}

foreach ($f in $tomlFiles) {
    if (Test-Path $f) {
        $lines = [System.IO.File]::ReadAllLines($f)
        for ($i = 0; $i -lt $lines.Count; $i++) {
            if ($lines[$i] -match '^version\s*=') {
                $lines[$i] = "version = `"$Version`""
                break
            }
        }
        [System.IO.File]::WriteAllLines($f, $lines, [System.Text.UTF8Encoding]::new($false))
        Write-Host "Synced $f -> version $Version"
    }
}
