# Zircon CI/CD & Build Guide: DO NOT DELETE

This guide outlines how to build, test, commit, and release the Zircon workspace (Launcher, Server Daemon, Core domain, and Website) locally and through **GitHub Actions**.

---

## 1. Golden Rule: Private Cloud Code Isolation

> [!CAUTION]
> **NEVER upload, commit, or push Zircon Cloud code to the public Git repository.**
> The cloud control plane and proprietary cloud orchestration code must remain strictly local/private.

### Excluded Paths & Files
The following paths are ignored in `.gitignore` and must **never** be staged:
* `cloud/` (Cloudflare Workers, control-plane backend, billing scripts)
* `crates/zircon-cloud/` (Internal cloud backend crate)
* `crates/zircon-server/src/cloud/` (Server node daemon cloud telemetry & RPC listener)
* `crates/zircon-server/src/web/controllers/migration_controller.rs` (1-click cloud migration controller)
* `*ZIRCON_CLOUD*` and `*PHASE_*` (Architecture and rollout planning docs)

### Ensuring Compilation Integrity for CI
Before pushing any commit to GitHub, verify that `crates/zircon-server` does **not** declare modules for files that are omitted from Git:
1. `crates/zircon-server/src/lib.rs` must **not** contain `pub mod cloud;`.
2. `crates/zircon-server/src/web/controllers/mod.rs` must **not** contain `pub mod migration_controller;`.
3. `Cargo.toml` workspace members must only contain:
   ```toml
   members = [
       "crates/zircon-core",
       "crates/zircon-server",
       "crates/zircon-launcher",
   ]
   ```

---

## 2. Local Builds

### Prerequisites
* **Rust** (stable toolchain: `rustc`, `cargo`)
* **Node.js** (v20+) & `npm`
* **NSIS** (optional, installed automatically via Tauri CLI for Windows installers)

### A. Testing the Workspace
To verify the entire workspace compiles without missing modules:
```powershell
cargo check --workspace
```
To run the automated test suite:
```powershell
cargo test --workspace
```

### B. Building the Launcher Frontend
Always re-compile the Vue UI when making changes to `crates/zircon-launcher/ui`:
```powershell
cd crates/zircon-launcher/ui
npm ci
npm run build
cd ../../..
```

### C. Compiling the Launcher Executable (.exe)
```powershell
cargo build --release -p zircon-launcher
```
The compiled binary will be located at:
```
target\release\zircon-launcher.exe
```

### D. Compiling the Full Windows NSIS Installer Bundle
To build the official Windows installer (.exe) with desktop shortcuts and WebView2 initialization:
```powershell
cd crates/zircon-launcher
npx --yes @tauri-apps/cli build --bundles nsis --config '{"bundle":{"createUpdaterArtifacts":false}}'
cd ../..
```
The installer output will be at:
```
target\release\bundle\nsis\Zircon_<version>_x64-setup.exe
```

### E. Compiling Zircon Server
```powershell
cargo build --release -p zircon-server
```
The server executable will be located at `target\release\zircon-server.exe`.

---

## 3. GitHub Actions CI/CD Release Pipeline

Zircon uses a unified multi-platform GitHub Actions workflow located at:
[`.github/workflows/build.yml`](file:///.github/workflows/build.yml)

### Workflow Triggers
The CI/CD pipeline triggers automatically on:
1. **Pushes to `main` or `master`** (runs compilation and artifact validation).
2. **Tags matching `v*` (e.g. `v0.4.11`)** (compiles, packages release binaries, publishes to GitHub Releases, and deploys website/artifacts).
3. **Manual Trigger (`workflow_dispatch`)** via the GitHub Actions UI tab.

### Matrix Platforms
GitHub Actions builds the following in parallel:
| Platform | Server Artifact | Launcher Artifacts |
| :--- | :--- | :--- |
| **Windows (x86_64)** | `zircon-server-windows-x86_64.zip` | `Zircon_<ver>_x64-setup.exe` (NSIS), `.msi` (WiX) |
| **Linux (x86_64)** | `zircon-server-linux-x86_64.tar.gz` | `.deb` package, `.AppImage` standalone |
| **macOS (Apple Silicon & Intel)** | `zircon-server-darwin-universal.tar.gz` | `Zircon_<ver>_universal.dmg`, `Zircon.app.tar.gz` |

---

## 4. How to Release a New Version

### Option A: Using `release.bat` (Recommended on Windows)
Run `release.bat` with the desired target version:
```cmd
release.bat 0.4.12
```
`release.bat` automatically:
1. Runs `scripts/sync-version.ps1` to synchronize the version across:
   * `crates/zircon-launcher/tauri.conf.json`
   * `crates/zircon-launcher/ui/package.json`
   * `crates/zircon-launcher/Cargo.toml`
   * `crates/zircon-server/Cargo.toml`
   * `crates/zircon-core/Cargo.toml`
   * `website/*.html`
2. Creates a Git commit (`Release v0.4.12`).
3. Creates a local tag (`v0.4.12`).
4. Pushes `main` and `--tags` to GitHub, which starts the GitHub Actions build.

---

### Option B: Manual Commit & Tag Push
If staging specific fixes manually:

1. **Sync Version Across Configs**:
   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\sync-version.ps1" "0.4.12"
   ```

2. **Verify Staged Files (Sanity Check for Cloud Code)**:
   ```powershell
   git status
   ```
   Ensure **no** files from `cloud/`, `crates/zircon-cloud/`, or `crates/zircon-server/src/cloud/` are staged.

3. **Verify Workspace Compiles**:
   ```powershell
   cargo check --workspace
   ```

4. **Commit & Tag**:
   ```powershell
   git add .gitignore Cargo.toml Cargo.lock crates/ website/
   git commit -m "Release v0.4.12"
   git tag -f "v0.4.12"
   ```

5. **Push to Trigger GitHub Actions**:
   ```powershell
   git push origin main --tags
   ```

6. **Monitor Live Progress**:
   Visit: **https://github.com/DevTheDev189/Zircon/actions**

---

## 5. Critical Troubleshooting & Lessons Learned

### Issue 1: `file not found for module <name>` on GitHub Actions
* **Cause**: A Rust file declared `pub mod foo;`, but `foo.rs` was not committed to Git (e.g. untracked or excluded by `.gitignore`).
* **Fix**: Ensure every module declared in `lib.rs` or `mod.rs` exists in the Git index before pushing.

### Issue 2: Launcher Window Remains Hidden in Background
* **Cause**: `tauri.conf.json` had `"visible": false` and the frontend awaited network/session restore before calling `win.show()`.
* **Fix**: Keep `"visible": true` in `tauri.conf.json` so the Tauri window renders immediately on boot.

### Issue 3: LaunchWrapper `ClassNotFoundException: net.minecraft.client.Minecraft` (Legacy Forge)
* **Cause**: Pre-1.13 Forge uses `minecraftArguments` containing `--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker`. If passed as a single string with spaces, Java's JOpt option parser fails to detect the flag and falls back to `VanillaTweaker`.
* **Fix**: Tokenize legacy arguments using `.split_whitespace()` so each flag and value is an individual command line argument (`argv[N]`).
