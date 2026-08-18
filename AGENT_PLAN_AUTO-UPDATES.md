# AGENT PLAN: Zircon Auto-Update Implementation

```markdown
# Task: Implement Auto-Update for Zircon Launcher & Zircon Server via Cloudflare R2

## Objective
Enable auto-updating for both the Tauri v2 desktop launcher (`zircon-launcher`) and the server daemon (`zircon-server`), hosting artifacts and manifests on `https://zirconmc.net/updates/`.

---

## Phase 1: Update `zircon-core` SSRF Domain Whitelist

### File: `crates/zircon-core/src/security/ssrf.rs`
Add `zirconmc.net` to `ALLOWED_CDN_DOMAINS` so that downloads from `https://zirconmc.net` pass safety verification.

```rust
pub const ALLOWED_CDN_DOMAINS: &[&str] = &[
    "zirconmc.net",
    "cdn.modrinth.com",
    "edge.forgecdn.net",
    "media.forgecdn.net",
    "maven.neoforged.net",
    "maven.minecraftforge.net",
    "meta.fabricmc.net",
    "meta.quiltmc.org",
    "piston-meta.mojang.com",
    "piston-data.mojang.com",
    "launcher.mojang.com",
    "launchermeta.mojang.com",
];
```

---

## Phase 2: Server Self-Update Engine (`zircon-server`)

### Step 2.1: Update `crates/zircon-server/Cargo.toml`
Add `self_replace`, `semver`, `sha2`, `hex` to the existing `[dependencies]` block (semver/sha2/hex are already workspace-level deps, so reference them with `.workspace = true`; `self_replace` is new and not in `[workspace.dependencies]`):
```toml
[dependencies]
zircon-core = { path = "../zircon-core" }

tokio = { workspace = true, features = ["full"] }
axum = { workspace = true }
tower-http = { workspace = true }
serde.workspace = true
serde_json.workspace = true
reqwest = { workspace = true }
bcrypt.workspace = true
jsonwebtoken.workspace = true
sysinfo.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
dashmap.workspace = true
bytes.workspace = true
uuid.workspace = true
chrono.workspace = true
url.workspace = true
zip.workspace = true
getrandom = "0.3"
tokio-util = { workspace = true, features = ["io"] }
futures-util = "0.3"
self-replace = "1.5"
semver.workspace = true
sha2.workspace = true
hex.workspace = true
```

### Step 2.2: Create `crates/zircon-server/src/updater.rs`
Create a self-updater module that handles downloading from `https://zirconmc.net/updates/server/latest.json`, verifying SHA-256 integrity, performing an atomic binary swap with `self_replace`, and respawning the process.

```rust
//! In-place binary self-updater for zircon-server.

use std::env;
use std::io::{Cursor, Read};
use std::process::Command;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zircon_core::security::ssrf;

pub const CURRENT_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SERVER_UPDATE_URL: &str = "https://zirconmc.net/updates/server/latest.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerUpdateManifest {
    pub version: String,
    pub release_date: String,
    pub notes: Option<String>,
    pub platforms: std::collections::HashMap<String, PlatformArtifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformArtifact {
    pub url: String,
    pub sha256: String,
    #[serde(rename = "binName")]
    pub bin_name: String,
}

pub struct ServerUpdater {
    client: reqwest::Client,
}

impl Default for ServerUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerUpdater {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// Checks if a newer version exists in the remote manifest.
    pub async fn check_update(&self) -> Result<Option<ServerUpdateManifest>, String> {
        let resp = self.client.get(SERVER_UPDATE_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to check update: {e}"))?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let manifest: ServerUpdateManifest = resp.json().await
            .map_err(|e| format!("Invalid update manifest: {e}"))?;

        let current = semver::Version::parse(CURRENT_SERVER_VERSION).map_err(|e| e.to_string())?;
        let target = semver::Version::parse(&manifest.version).map_err(|e| e.to_string())?;

        if target > current {
            Ok(Some(manifest))
        } else {
            Ok(None)
        }
    }

    /// Downloads the archive, verifies SHA-256, extracts the binary, and swaps it in place.
    pub async fn apply_update(&self, manifest: &ServerUpdateManifest) -> Result<(), String> {
        let platform_key = if cfg!(target_os = "windows") {
            "windows-x86_64"
        } else if cfg!(target_os = "linux") {
            "linux-x86_64"
        } else if cfg!(target_os = "macos") {
            "macos-x86_64"
        } else {
            return Err("Unsupported OS platform for auto-update".into());
        };

        let artifact = manifest.platforms.get(platform_key)
            .ok_or_else(|| format!("No release available for platform {platform_key}"))?;

        // 1. Validate domain security
        if !ssrf::is_safe_cdn_url(&artifact.url) {
            return Err(format!("Untrusted update source host: {}", artifact.url));
        }

        // 2. Download compressed binary archive
        let bytes = self.client.get(&artifact.url)
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("Read failed: {e}"))?;

        // 3. Verify SHA256 Checksum
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let calculated_hash = hex::encode(hasher.finalize());
        if !calculated_hash.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(format!(
                "Checksum mismatch! Expected {}, got {}",
                artifact.sha256, calculated_hash
            ));
        }

        // 4. Extract new binary in memory
        let cursor = Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor).map_err(|e| format!("Corrupt zip: {e}"))?;
        let mut new_bin_bytes = Vec::new();
        let mut file = zip.by_name(&artifact.bin_name)
            .map_err(|_| format!("Binary '{}' not found inside archive", artifact.bin_name))?;
        file.read_to_end(&mut new_bin_bytes).map_err(|e| e.to_string())?;

        // 5. Atomic self replace on disk
        let temp_bin_path = std::env::temp_dir().join(format!("zircon_update_{}", manifest.version));
        std::fs::write(&temp_bin_path, &new_bin_bytes).map_err(|e| e.to_string())?;

        self_replace::self_replace(&temp_bin_path)
            .map_err(|e| format!("Failed to swap executable: {e}"))?;
        let _ = std::fs::remove_file(temp_bin_path);

        tracing::info!("Server binary successfully updated to v{}.", manifest.version);
        Ok(())
    }

    /// Relaunches the updated binary and exits current process.
    pub fn restart_process() -> Result<(), std::io::Error> {
        let current_exe = env::current_exe()?;
        let args: Vec<String> = env::args().skip(1).collect();

        Command::new(current_exe)
            .args(&args)
            .spawn()?;

        std::process::exit(0);
    }
}
```

### Step 2.3: Register Module in `crates/zircon-server/src/lib.rs`
Add `pub mod updater;` to `crates/zircon-server/src/lib.rs`.

### Step 2.4: Create `crates/zircon-server/src/web/controllers/system_controller.rs`
```rust
//! System update and health controller.

use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::updater::{ServerUpdater, CURRENT_SERVER_VERSION};
use crate::web::app::{ApiError, AppState};

/// GET /api/system/update/check — Checks if a server update is available.
pub async fn check_update() -> Result<Json<serde_json::Value>, ApiError> {
    let updater = ServerUpdater::new();
    let update = updater.check_update().await.map_err(ApiError::Internal)?;
    Ok(Json(json!({
        "currentVersion": CURRENT_SERVER_VERSION,
        "updateAvailable": update.is_some(),
        "manifest": update
    })))
}

/// POST /api/system/update/apply — Downloads and replaces the server binary, then restarts.
pub async fn apply_update(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updater = ServerUpdater::new();
    let Some(manifest) = updater.check_update().await.map_err(ApiError::Internal)? else {
        return Err(ApiError::BadRequest("No updates available".into()));
    };

    // 1. Gracefully stop all Minecraft server instances before replacing
    for inst in state.instances.list_instances() {
        state.instances.stop_instance(&inst.id).await;
    }

    // 2. Perform binary replacement
    updater.apply_update(&manifest).await.map_err(ApiError::Internal)?;

    // 3. Spawn background restart
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let _ = ServerUpdater::restart_process();
    });

    Ok(Json(json!({
        "ok": true,
        "message": "Server updated successfully. Restarting..."
    })))
}
```

### Step 2.5: Register in `crates/zircon-server/src/web/controllers/mod.rs` & `crates/zircon-server/src/web/app.rs`
1. In `crates/zircon-server/src/web/controllers/mod.rs`, add `pub mod system_controller;`.
2. In `crates/zircon-server/src/web/app.rs`, add `system_controller` to the existing `use super::controllers::{...};` import list, then add the routes to `protected_api`:
```rust
use super::controllers::system_controller;

// Inside `protected_api` Router:
.route("/api/system/update/check", get(system_controller::check_update))
.route("/api/system/update/apply", post(system_controller::apply_update))
```

---

## Phase 3: Launcher Auto-Update (`zircon-launcher`)

### Step 3.1: Update `crates/zircon-launcher/Cargo.toml`
Add to the existing `[dependencies]` block:
```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

### Step 3.2: Update `crates/zircon-launcher/src/lib.rs`
Register the plugins (added alongside the existing `tauri_plugin_dialog::init()` call):
```rust
tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    .manage(commands::LauncherState::new())
    // ...
```

### Step 3.3: Configure `crates/zircon-launcher/ui/package.json`
Bump `@tauri-apps/api` (currently pinned to `^2.1.1`) and add the updater/process packages:
```json
"dependencies": {
  "@tauri-apps/api": "^2.11.1",
  "@tauri-apps/plugin-dialog": "^2.0.1",
  "@tauri-apps/plugin-updater": "^2.0.0",
  "@tauri-apps/plugin-process": "^2.0.0",
  "three": "^0.170.0",
  "vue": "^3.5.13"
}
```

### Step 3.4: Grant Tauri v2 Permissions in `crates/zircon-launcher/capabilities/default.json`
Tauri v2 denies plugin commands at runtime unless explicitly capability-granted. The
existing file only grants `core:default` and `dialog:default`; add the updater and
process defaults:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capability for the Zircon launcher main window",
  "windows": ["main"],
  "permissions": ["core:default", "dialog:default", "updater:default", "process:default"]
}
```

### Step 3.5: Add the Updater Config Block to `crates/zircon-launcher/tauri.conf.json`
The file currently has no `plugins` key at all. Add one with the update endpoint and
the pubkey generated in the post-implementation steps below (leave `pubkey` as an
empty string placeholder — it must be filled in after `tauri signer generate` runs,
the app will fail to start with a real endpoint configured and an invalid/empty key
only if `active: true`, so keep `active: false` until the key is in place):
```json
{
  "...": "...",
  "bundle": {
    "...": "...",
    "createUpdaterArtifacts": true
  },
  "plugins": {
    "updater": {
      "active": false,
      "endpoints": [
        "https://zirconmc.net/updates/launcher/latest.json"
      ],
      "pubkey": ""
    }
  }
}
```
`plugins.updater` controls the frontend's runtime update check. `bundle.createUpdaterArtifacts`
is a *separate* key (defaults to `false`) that tells the bundler to actually produce the
signed `.sig` files and `updater/latest.json` during `tauri build` — without it, `tauri build`
silently skips update-artifact generation even with a valid pubkey and signing key configured,
and no error is printed. Both are required.

Flip `plugins.updater.active` to `true` once the pubkey is filled in (Step 1 of the
post-implementation checklist below).

### Step 3.6: Add Launcher Update Check in Vue Frontend
In `crates/zircon-launcher/ui/src/App.vue` (or wherever app initialization occurs), add:
```javascript
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

async function checkLauncherUpdate() {
  try {
    const update = await check();
    if (update?.available) {
      console.log(`Update ${update.version} available!`);
      let downloaded = 0;
      let contentLength = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          contentLength = event.data.contentLength || 0;
        } else if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
        }
      });
      await relaunch();
    }
  } catch (err) {
    console.warn('Launcher update check failed:', err);
  }
}
```

---

## Phase 4: Update `build.bat` Pipeline

### File: `build.bat`
Update the build script so it automatically packages the server, computes the SHA-256 hash, and emits the `server-latest.json` alongside the Tauri release bundles:

```bat
@echo off
rem ===========================================================================
rem  Zircon build & release packaging script
rem  Compiles server and launcher, generates updater metadata and packages.
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
echo   - dist-run\server-latest.json -> https://zirconmc.net/updates/server/latest.json
echo   - dist-run\zircon-server-windows-x86_64.zip -> https://zirconmc.net/updates/server/v%VERSION%/zircon-server-windows-x86_64.zip
echo   - dist-run\launcher-latest.json -> https://zirconmc.net/updates/launcher/latest.json
echo   - target\release\bundle\nsis\Zircon_%VERSION%_x64-setup.exe -> https://zirconmc.net/updates/launcher/Zircon_%VERSION%_x64-setup.exe
echo   - (optional, unsigned-feed installers) target\release\bundle\msi\*.msi -> https://zirconmc.net/updates/launcher/
echo ===========================================================================
endlocal
```
`RemoteRelease` deserialization (`tauri-plugin-updater` v2.10.1, `src/updater.rs`) requires
`version` and a `platforms` map with `{url, signature}` per platform key; `pub_date`, if
present, MUST be strict RFC3339 (`OffsetDateTime::parse(..., Rfc3339)`) or the whole manifest
fails to parse — that's why the script shells out to PowerShell's `Get-Date` rather than using
batch's locale-dependent `%DATE% %TIME%` (fine for the server manifest, which reads its
`releaseDate` as an untyped `String`, but not safe here).

---

## Phase 5: Verification, Compilation, Commit & Push

1. Run `cargo check --workspace` and `cargo test --workspace` to ensure zero compilation or unit test regressions.
2. Stage all changed and new files:
   ```bash
   git add crates/zircon-core/ crates/zircon-server/ crates/zircon-launcher/ build.bat
   ```
3. Commit with a descriptive message:
   ```bash
   git commit -m "feat: implement Cloudflare R2 auto-updater for launcher and server"
   ```
4. Push to origin:
   ```bash
   git push origin HEAD
   ```
```

---

### What You Need to Do Once the Agent Plan is Done

Once the coding agent has implemented the code, committed, and pushed:

1. **Generate your Tauri Signing Key**:
   Run this on your development machine:
   ```bash
   npx @tauri-apps/cli signer generate -w ~/.tauri/zircon.key
   ```
   * It will output a **Public Key**. Copy it into `crates/zircon-launcher/tauri.conf.json` under `plugins.updater.pubkey` (there is no `src-tauri/` subdirectory in this workspace — the Tauri config lives directly under `crates/zircon-launcher/`), then flip `plugins.updater.active` to `true` AND add `"createUpdaterArtifacts": true` under `bundle` (see Step 3.5 above — this is a separate, easy-to-miss key; without it `tauri build` silently produces no `.sig` files, no error).
   * Set the private key content (not the file path — this build's Tauri CLI version only honors the raw-content env var, `TAURI_SIGNING_PRIVATE_KEY_PATH` is documented by `signer generate` but not actually read by the bundler's signing step) before building:
     * **Windows Command Prompt**: `set TAURI_SIGNING_PRIVATE_KEY=<key_content>` (contents of `~/.tauri/zircon.key`, not the `.pub` file). If the key has a password, also set `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. If it was generated with `--ci` and no `-p`, it has no password — `tauri build` will still prompt for one at decrypt time, and an empty/EOF answer is accepted, which is what happens automatically in a non-interactive shell.

2. **Run `build.bat`**:
   This builds the binaries, packages the server zip, and produces the signed MSI/NSIS
   installers plus their `.sig` files. It also hand-generates `dist-run/launcher-latest.json`
   itself (see Phase 4) since the Tauri CLI does not emit an `updater/latest.json` manifest
   for self-hosted feeds — only `tauri-action` in GitHub Actions does that automatically.

3. **Upload the Files to your R2 Bucket**:
   Upload the generated artifacts to the matching locations under your bucket's `/updates/` path:
   * `dist-run/server-latest.json` $\rightarrow$ `updates/server/latest.json`
   * `dist-run/zircon-server-windows-x86_64.zip` $\rightarrow$ `updates/server/v0.1.0/zircon-server-windows-x86_64.zip`
   * `dist-run/launcher-latest.json` $\rightarrow$ `updates/launcher/latest.json`
   * `target/release/bundle/nsis/Zircon_0.1.0_x64-setup.exe` $\rightarrow$ `updates/launcher/Zircon_0.1.0_x64-setup.exe`
   * (optional) `target/release/bundle/msi/*.msi` $\rightarrow$ `updates/launcher/` — not referenced by `launcher-latest.json`, only useful as a direct-download alternative to the NSIS installer.
