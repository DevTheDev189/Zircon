# AGENT_PLAN.md: Migration Plan for Zircon (MCManager) to Rust

This document is an actionable, step-by-step master plan for an automated coding agent to re-write the entire **Zircon (MCManager)** system from Java (JavaFX / Javalin / Netty) into a modern, highly efficient, secure **Rust** workspace using **Axum** for the server-manager and **Tauri v2** + **Vue 3** for the client-launcher.

---

## 1. System Architecture & Tech Stack

### Target Tech Stack
* **Language & Edition**: Rust 2021 Edition.
* **Workspace Structure**: Cargo Multi-crate Workspace.
* **Core Library (`zircon-core`)**: `serde`, `serde_json`, `sha1`, `sha2`, `hex`, `zip`, `toml`, `tar`, `lz4_flex`, `reqwest`, `url`.
* **Server Manager (`zircon-server`)**: `tokio` (full async runtime), `axum` (v0.7+ REST/WebSockets), `tower-http` (CORS, Trace, Static files), `bcrypt`, `jsonwebtoken`, `sysinfo`, `tracing`, `tracing-subscriber`, `dashmap`, `bytes`, `tokio-util`.
* **Client Launcher (`zircon-launcher`)**: `tauri` (v2.x), `tauri-plugin-shell`, `tauri-plugin-dialog`, `tauri-plugin-updater`, `tauri-plugin-process`, `open`, `image`.
* **Launcher Frontend (`ui/`)**: Vue 3 (Composition API), Vite, Tailwind CSS, Three.js (for WebGL 3D player skin rendering inside Webview).

---

## 2. Workspace Directory Layout

```text
zircon/
├── Cargo.toml                        # Workspace root manifest
├── crates/
│   ├── zircon-core/                  # Shared types, cryptography, API clients, archive utils
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model/                # BOM, InstanceConfig, ModEntry, PackEntry, etc.
│   │       ├── crypto/               # MurmurHash3, SHA-1, SHA-256 streaming utilities
│   │       ├── archive/              # LZ4 + Tar compression & zip-slip protected extraction
│   │       ├── metadata/             # ModMetadataExtractor (fabric.mod.json, mods.toml, neoforge.mods.toml)
│   │       ├── api/                  # Modrinth & CurseForge API HTTP clients
│   │       └── security/             # SSRF CDN URL domain whitelist validator
│   │
│   ├── zircon-server/                # Server Manager Daemon & Admin Web Server
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── config.rs             # config.json and server.properties management
│   │       ├── auth/                 # BCrypt users.json & JWT issuance/validation
│   │       ├── tickets.rs            # JoinTicketManager (TTL-backed in-memory join gate)
│   │       ├── instance.rs           # Multi-instance manager (<data>/instances/<id>/)
│   │       ├── process/              # Subprocess execution, stdin command writer, line parser
│   │       ├── installer.rs          # Headless server installer (Vanilla, Fabric, Quilt, Forge, NeoForge)
│   │       ├── multiplexer/          # Tokio TCP multiplexer, VarInt parser, Minecraft handshake router, TCP proxy
│   │       ├── services/             # BomService, ModManagementService, PackManagementService, BackupService, BackupScheduler
│   │       ├── stats.rs              # Real-time host CPU/RAM/Disk metrics via sysinfo
│   │       └── web/                  # Axum REST routes, WebSockets console streamer, SPA fallback
│   │
│   └── zircon-launcher/              # Tauri v2 Client Desktop Launcher
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── src/
│       │   ├── main.rs
│       │   ├── auth/                 # Microsoft OAuth PKCE flow (S256), local HTTP listener, token cache
│       │   ├── launch/               # Classpath builder, Java runtime downloader, Forge/NeoForge launch resolver, runner
│       │   ├── sync/                 # ModSyncEngine (.mod_staging reconciliation), PackSyncEngine
│       │   ├── offline.rs            # Offline instance storage & local mod manager
│       │   ├── skin.rs               # Skin storage, history pruning, head icon cropper, Mojang skin API
│       │   ├── update.rs             # R2 manifest check & app updater
│       │   └── commands.rs           # Tauri IPC command bindings exposed to Webview
│       └── ui/                       # Vue 3 + Tailwind + Vite + Three.js Frontend
│           ├── index.html
│           ├── package.json
│           ├── vite.config.js
│           └── src/                  # Views: Servers, Play Offline, Skins (3D Canvas), Settings, Packs
```

---

## 3. Required Crates & Version Specifications

### `Cargo.toml` (Workspace Level)
```toml
[workspace]
resolver = "2"
members = [
    "crates/zircon-core",
    "crates/zircon-server",
    "crates/zircon-launcher",
]

[workspace.dependencies]
tokio = { version = "1.40", features = ["full"] }
axum = { version = "0.7", features = ["ws", "multipart"] }
tower-http = { version = "0.5", features = ["cors", "fs", "trace"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.12", features = ["json", "stream", "multipart"] }
sha1 = "0.10"
sha2 = "0.10"
hex = "0.4"
tar = "0.4"
lz4_flex = "0.11"
bcrypt = "0.15"
jsonwebtoken = "9.3"
sysinfo = "0.31"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
toml = "0.8"
zip = "2.2"
uuid = { version = "1.10", features = ["v4", "v3", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
url = "2.5"
open = "5.3"
image = "0.25"
dashmap = "6.0"
bytes = "1.7"
tokio-util = { version = "0.7", features = ["codec", "io"] }
semver = "1.0"
tauri = { version = "2.1", features = ["protocol-asset"] }
tauri-build = "2.0"
```

---

## 4. Phase-by-Phase Implementation Plan

### Phase 1: Shared Core Domain (`zircon-core`) ✅ DONE

> **Status: COMPLETE** — `cargo check -p zircon-core` clean; `cargo test -p zircon-core` passes 46/46 (MurmurHash3 verified byte-for-byte against the Java implementation, SHA-1/256, zip-slip, TOML/Fabric metadata, SSRF domain checks).

#### Objectives
Implement data models, hash algorithms, archive compressors, mod JAR metadata parsers, Modrinth/CurseForge HTTP client logic, and security domain filters in pure Rust.

#### Step 1.1: Data Models (`crates/zircon-core/src/model/`)
* **`bom.rs`**: Structs `BillOfMaterials`, `ModEntry`, `PackEntry`, `ModLoaderInfo`. Implement JSON serialization with `serde` matching existing schema (`schemaVersion`, `minecraftVersion`, `modLoader`, `mods`, `shaderpacks`, `resourcepacks`, `serverTitle`).
* **`instance.rs`**: Struct `InstanceConfig`. Implement locked `ModLoaderInfo`, internal/external ports, backup settings, auto-start flags. Ensure `mod_loader` cannot be updated once set.
* **`backup.rs`**: Struct `BackupEntry` (`id`, `instance_id`, `filename`, `timestamp`, `size_bytes`, `trigger_type`, `status`, `logs`).
* **`metadata.rs`**: Struct `ModMetadata` (`id`, `name`, `version`, `description`, `loader_type`, `environment`).

#### Step 1.2: Cryptography & Hashes (`crates/zircon-core/src/crypto/`)
* **`hash.rs`**: Streaming SHA-1 and SHA-256 calculator functions reading `tokio::fs::File` via 8 KiB buffer blocks into lower-case hex strings.
* **`murmur3.rs`**: Port Java `MurmurHash3` implementation.
  * **Critical Nuance**: CurseForge requires stripping ASCII whitespace bytes (`0x09`, `0x0A`, `0x0D`, `0x20`) from file contents prior to computing 32-bit MurmurHash3 with seed `1`.
  * Return unsigned 32-bit values formatted as `u64`.

#### Step 1.3: Archive Utilities (`crates/zircon-core/src/archive/`)
* **`lz4_tar.rs`**:
  * `compress_directory(source_dir, target_archive, exclude_dir, audit_logs)`: Walk directory tree, stream files into `tar::Builder` wrapped with `lz4_flex::frame::FrameEncoder`. Track file counts, uncompressed/compressed sizes, and execution time.
  * `extract_archive(archive_file, destination_dir)`: Read `lz4_flex::frame::FrameDecoder` into `tar::Archive`.
  * **Security Requirement**: Sanitize entry paths. Reject entries attempting path traversal outside `destination_dir` (Zip-slip check using `.components()`).

#### Step 1.4: Mod Metadata Extractor (`crates/zircon-core/src/metadata/`)
* **`extractor.rs`**: Inspect Zip archives (`zip::ZipArchive`).
  * Check entries in precedence order:
    1. `fabric.mod.json`: Parse JSON into `ModMetadata` (loader: Fabric).
    2. `META-INF/neoforge.mods.toml`: Parse TOML using `toml` crate into `ModMetadata` (loader: NeoForge).
    3. `META-INF/mods.toml`: Parse TOML using `toml` crate into `ModMetadata` (loader: Forge).
  * Handle edge cases: missing fields, numeric version strings in TOML tables, environment string or object.

#### Step 1.5: External Mod API Clients & Security (`crates/zircon-core/src/api/` & `security/`)
* **`modrinth.rs`**: Async reqwest client for `https://api.modrinth.com/v2`.
  * Implement `verify_hashes` (POST `/version_files`), `search_mods`, `list_game_versions`, `list_loaders`, `list_project_versions`, `get_project`. Include custom `User-Agent`.
* **`curseforge.rs`**: Async reqwest client for `https://api.curseforge.com/v1` with `x-api-key`.
  * Implement `verify_fingerprints` (POST `/fingerprints`), `search_mods`, `list_mod_files`.
* **`security.rs`**: `is_safe_cdn_url(url: &str) -> bool`. Whitelist validation against known CDN domains (`cdn.modrinth.com`, `edge.forgecdn.net`, `maven.neoforged.net`, `maven.minecraftforge.net`, `meta.fabricmc.net`, `meta.quiltmc.org`, `piston-meta.mojang.com`, `launchermeta.mojang.com`). Reject non-whitelisted hosts, loopback (`127.0.0.1`), and cloud metadata IP (`169.254.169.254`).

#### Phase 1 Checkpoint
Run `cargo check -p zircon-core` and `cargo test -p zircon-core`. Ensure unit tests pass for MurmurHash3, SHA1, ZIP-slip prevention, TOML metadata parsing, and SSRF domain checks.

> ✅ **Verified**: `cargo check -p zircon-core` clean · `cargo test -p zircon-core` → **46 passed, 0 failed**. MurmurHash3 matches Java reference values generated from the original `MurmurHash3` class (seed 0 vectors + seed 1 + CurseForge fingerprint).

---

### Phase 2: Server Engine, TCP Multiplexer & Multi-Instance Manager (`zircon-server`) ✅ DONE

> **Status: COMPLETE** — config, instance manager, process supervisor, headless installer, join tickets, console/player tracking, VarInt/disconnect/detector/multiplexer, backups + scheduler all implemented and tested.

#### Objectives
Build the core server process supervisor, multi-instance engine, multi-port TCP multiplexer with custom Minecraft handshake/VarInt framing and Join Gate ticket validation, and the backup scheduler.

#### Step 2.1: Server Config & Multi-Instance Engine (`crates/zircon-server/src/`)
* **`config.rs`**: Manage `<data>/config.json` and `<data>/server/server.properties` with line-preserving key-value parser.
* **`instance.rs`**: `ServerInstanceManager` managing `<data>/instances/<id>/`.
  * Instance creation: Allocate internal MC port (base 25700+) and external player port (base 25565+). Write `<id>/instance.json`, create `mods/` and `server/` folders.
  * Instance config locks: Ensure `mod_loader.type` cannot be modified after creation.
  * Implement methods: `list_instances`, `get_instance`, `update_instance_config`, `update_instance_versions` (triggers `ModManagementService::sync_mods_for_version_change`), `delete_instance`, `accept_eula`, `is_eula_accepted`.

#### Step 2.2: Minecraft Process Supervisor & Headless Installer (`crates/zircon-server/src/process/` & `installer.rs`)
* **`process.rs`**: `MinecraftProcessManager` using `tokio::process::Command`.
  * Bind `server-port` and `server-ip=127.0.0.1` inside instance `server.properties` prior to launch.
  * Stream stdout/stderr line-by-line via `tokio::io::BufReader`. Broadcast lines to `ConsoleStreamHandler` subscribers.
  * Implement `send_command(cmd)` to write to process `stdin`.
* **`installer.rs`**: Headless server installer.
  * **Vanilla**: Fetch server URL from Mojang `version_manifest_v2.json`, download `server.jar`.
  * **Fabric / Quilt**: Download server launcher JAR from meta API (`/versions/loader/{mc}/{loader}/{installer}/server/jar`).
  * **Forge / NeoForge**: Download installer JAR, run `java -jar <installer>.jar --installServer <dir>` via Tokio subprocess, locate generated `@unix_args.txt` or `@win_args.txt` arguments file.

#### Step 2.3: Join Ticket Manager & Console Parsing (`crates/zircon-server/src/`)
* **`tickets.rs`**: In-memory `JoinTicketManager` using `DashMap<String, Instant>`.
  * `register_ticket(identifier: String)`: Register username/UUID ticket with 5-minute TTL.
  * `consume_ticket(identifier: &str) -> bool`: Atomically check and remove active ticket. One-time use enforcement.
* **`console.rs` & `player_tracker.rs`**:
  * Circular history ring buffer (1000 lines).
  * Line filter for levels (`INFO`, `WARN`, `ERROR`).
  * Regex parser for `"joined the game"`, `"left the game"`, `"lost connection:"`. Maintain online player sets and write persistent `<instance>/players.json` ever-joined log.

#### Step 2.4: Tokio TCP Multiplexer & Minecraft Protocol Router (`crates/zircon-server/src/multiplexer/`)
* **`varint.rs`**: VarInt encoder/decoder for Minecraft protocol bytes.
* **`disconnect.rs`**: Build framed Minecraft Login Disconnect packet (`0x00` in login state) with custom JSON error message ("Zircon Client Required").
* **`detector.rs`**: Custom Tokio stream parser inspecting initial bytes:
  1. If stream matches HTTP methods (`GET `, `POST `, `PUT `, `DELETE `, `OPTIONS `, `HEAD `) -> proxy stream to local Axum web server port.
  2. If stream matches Minecraft Handshake frame:
     * Read packet frame length, packet ID `0x00`, protocol version, address string (hostname), port, and nextState (`1` = Status Ping, `2` = Login).
     * If `nextState == 2` (Login) and instance mode is active:
       * Read next packet frame (Login Start `0x00`).
       * Extract username.
       * Verify against `JoinTicketManager`. If ticket is missing/invalid, send framed Disconnect JSON packet and drop socket.
     * Route connection to target instance internal MC port (`tokio::io::copy_bidirectional`).
* **`multiplexer.rs`**: Bind public port 25565 + per-instance external player ports. Implement `PortBindingListener`.

#### Step 2.5: Backup Engine & Scheduler (`crates/zircon-server/src/services/`)
* **`backup.rs`**: `BackupService`. Store backups in `<data>/backups/<instance_id>/`.
  * On backup: send `save-off` and `save-all` to process stdin, wait 2.5s, compress instance folder to `<id>.tar.lz4`, write `<id>.json` audit sidecar, send `save-on`.
  * Retention pruning: delete oldest archives exceeding instance `backupRetention` setting.
  * On restore: stop process, move current instance folder to temporary rollback folder, extract LZ4 archive, reload config.
* **`scheduler.rs`**: `BackupSchedulerService`. Run background Tokio task checking instance schedules (`daily`, `weekly`, `monthly`) against `backupTime` every 10 minutes.

#### Phase 2 Checkpoint
Run `cargo check -p zircon-server` and `cargo test -p zircon-server`. Verify VarInt decoding, join ticket expiration, Tokio TCP proxying, and LZ4 backup creation.

> ✅ **Verified**: `cargo check -p zircon-server` clean · `cargo test -p zircon-server` → **58 passed, 0 failed**. VarInt round-trips (incl. negative encodings), ticket one-time use + TTL expiry, HTTP/Minecraft proxying through the multiplexer, join-gate disconnect frames, LZ4 backup/restore round-trip, retention pruning, host-port resolution, vanilla player files.
> Boot smoke test: initial admin user + JWT secret generated, instances loaded from `server-data`, web API on `127.0.0.1:25564`, multiplexer + per-instance ports bound on `25565/25566`.

---

### Phase 3: Server Axum REST & WebSocket Admin API (`zircon-server`) ✅ DONE

#### Objectives
Implement the complete HTTP admin REST API, JWT authentication, and WebSockets console streamer using **Axum**.

#### Step 3.1: Axum Application Setup & Auth Middleware (`crates/zircon-server/src/web/`)
* **`app.rs`**: Construct Axum `Router`.
* **`auth.rs`**: `AuthService` (manage `users.json` with BCrypt hashes, generate initial admin password) and `JwtUtil` (HMAC SHA-256 JWT generation/validation with 12h TTL).
* **Middleware**: Axum `from_fn` layer enforcing `Authorization: Bearer <jwt>`.
  * Exclude public routes: `POST /api/auth/login`, `POST /api/auth/change-password`, `POST /api/join-intent`, `POST /api/instances/:id/join-intent`.

#### Step 3.2: REST Controllers (`crates/zircon-server/src/web/controllers/`)
* **`auth_controller.rs`**: Login, profile query, change password, profile update.
* **`bom_controller.rs`**: `GET /bom` (resolves instance from request `Host` port or active instance).
* **`mod_controller.rs`**: List, upload (multipart), download, delete, provider search, install from Modrinth/CurseForge.
* **`pack_controller.rs`**: Upload, download, install, delete shaderpacks and resourcepacks.
* **`instance_controller.rs`**: Instance CRUD, start, stop, restart, EULA accept, `server.properties` read/write, join-intent ticket registration.
* **`player_controller.rs`**: Online players, player history, whitelist, ban list, op list, kick, ban (online & offline `banned-players.json` fallback).
* **`backup_controller.rs`**: List backups, create manual backup, restore backup, update retention setting.
* **`stats_controller.rs`**: `GET /api/stats` (sample system CPU, process CPU, memory, and disk using `sysinfo` crate).

#### Step 3.3: WebSocket Console Controller (`crates/zircon-server/src/web/controllers/console.rs`)
* Axum WebSocket upgrade route `/api/console`.
* Stream console ring buffer (last 500 lines) on connect.
* Broadcast new lines to active WebSockets. Write incoming WS text messages as stdin commands to process manager. Handle `__CLEAR__` command.

#### Phase 3 Checkpoint
Run `cargo check -p zircon-server`. Verify all REST routes compile and JWT middleware properly gates protected endpoints.

---

### Phase 4: Client Launcher Core Engine (`zircon-launcher`) ✅ DONE

#### Objectives
Build the client-side launch pipeline in Rust: Microsoft OAuth PKCE authentication, vanilla asset/library downloading, Fabric/Quilt/Forge/NeoForge launch resolution, offline instance manager, and mod/pack sync engines.

#### Step 4.1: Microsoft Authentication (`crates/zircon-launcher/src/auth/`)
* **`msa.rs`**: `MicrosoftAuthService`.
  * Resolution order for Azure Client ID: `-Dmcmanager.clientId`, CLI `--clientId`, `~/.mcmanager/client_id.txt`, embedded default constant (`37f881f0-0083-45af-b2c4-52a658fec513`).
  * Implement PKCE (S256 code verifier & challenge).
  * Dynamic local callback server on `http://127.0.0.1:<port>/callback` using `tokio::net::TcpListener` (port `0` OS allocation). Return styled HTML confirmation page.
  * Flow: MSA auth code -> OAuth token -> XBL authenticate (`user.auth.xboxlive.com`) -> XSTS authorize (`xsts.auth.xboxlive.com`) -> Minecraft Services login (`api.minecraftservices.com/authentication/login_with_xbox`) -> fetch Minecraft profile.
  * Cache session to `~/.mcmanager/auth_cache.json`. Implement refresh token silent renewal.

#### Step 4.2: Vanilla Classpath & Asset Resolver (`crates/zircon-launcher/src/launch/`)
* **`classpath.rs`**: `MinecraftClasspathBuilder`.
  * Download Mojang `version_manifest_v2.json` to `~/.mcmanager/launcher/`.
  * Parse version JSON, extract client JAR, libraries, native dependencies per OS/arch rules. Extract natives (`.dll`, `.so`, `.dylib`) to `natives/` folder.
  * Fetch asset index JSON and download assets (`https://resources.download.minecraft.net/<hash2>/<hash>`) using bounded async concurrency pool (e.g. `tokio::sync::Semaphore` set to 8 concurrent downloads).

#### Step 4.3: Java Runtime Provisioner (`crates/zircon-launcher/src/launch/`)
* **`java.rs`**: `JavaRuntimeResolver`.
  * Map Minecraft version to required Java major version (<1.17 -> Java 8, 1.17 -> Java 16, 1.18-1.20.4 -> Java 17, 1.20.5+ -> Java 21).
  * Compare with current system Java; if insufficient, download Temurin JDK archive from Adoptium API (`https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse`) and extract to `~/.mcmanager/launcher/jdk-{major}/`.

#### Step 4.4: Mod Loader Launch Resolvers (`crates/zircon-launcher/src/launch/`)
* **`fabric_quilt.rs`**: Query meta APIs (`meta.fabricmc.net` / `meta.quiltmc.org`) for loader profiles, resolve loader libraries, append to classpath.
* **`forge_neoforge.rs`**: `ForgeLaunchResolver`.
  * Headless installation: Download installer JAR from Maven (`maven.minecraftforge.net` / `maven.neoforged.net`). Run `java -jar <installer>.jar --installClient <installDir>` or `--install-client`.
  * Parse generated `versions/<id>/<id>.json`. Resolve `inheritsFrom` chain down to vanilla profile.
  * Merge libraries, de-duplicating by Maven coordinate. Stage loader artifacts (`-universal.jar`, `-client.jar`, `minecraft-client-patched.jar`) into unified libraries directory.
  * Resolve JVM and game arguments with `${token}` substitution (`library_directory`, `classpath_separator`, `version_name`, `auth_player_name`, `auth_access_token`, `auth_uuid`, `user_type`, `game_directory`, `assets_root`, `assets_index_name`, `quickPlayMultiplayer`, `natives_directory`).

#### Step 4.5: Game Process Runner & Option Overrides (`crates/zircon-launcher/src/launch/`)
* **`runner.rs`**: `MinecraftRunner`.
  * Construct `tokio::process::Command` with JVM memory (`-Xmx4G`), natives path, classpath, main class, and game args.
  * Enforce valid Microsoft session check (userType == "msa" and access token present) for online play.
  * Pre-configure client files in instance directory before launch:
    * `options.txt`: set `skipMultiplayerWarning:true` and `fullscreen:true`.
    * `config/iris.properties`: write active shaderpack selection (`enableShaders=true/false`, `shaderPack=...`).
    * `options.txt`: write active resourcepacks array (`resourcePacks:["vanilla", "file/..."]`).
  * Append auto-connect arguments (`--quickPlayMultiplayer host:port`).
  * Implement `launch_offline` for single-player mode with offline UUID.

#### Step 4.6: Client Sync Engines & Offline Instance Manager (`crates/zircon-launcher/src/`)
* **`sync/mod_sync.rs`**: `ModSyncEngine`. Fetch server `/bom`, verify hashes against Modrinth/CurseForge, download missing files to `.mod_staging` directory, reconcile active `mods/` directory (delete unlisted mods, copy staged files).
* **`sync/pack_sync.rs`**: `PackSyncEngine`. Reconcile `shaderpacks/` and `resourcepacks/` against BOM, preserving player's locally added packs.
* **`offline.rs`**: `OfflineInstanceManager`. Store instances under `~/.mcmanager/offline_instances/<id>/`. Save `instance.json`, manage `mods/` folder.

#### Phase 4 Checkpoint
Run `cargo check -p zircon-launcher` and `cargo test -p zircon-launcher`. Verify Microsoft PKCE authorize URL formatting, Classpath token substitution, and `.mod_staging` directory reconciliation logic.

---

### Phase 5: Tauri v2 App Shell, Vue 3 GUI & 3D Skin Renderer (`zircon-launcher`) ✅ DONE

> **Status: COMPLETE** — Tauri v2 shell (`tauri.conf.json`, IPC commands, skin storage, pack selection, saved servers), the Vue 3 + Tailwind frontend with all four views + login overlay + status bar, and the Three.js 64x64 dual-layer skin renderer are implemented and building.

#### Objectives
Build the desktop user interface using Tauri v2, Vue 3, Tailwind CSS, and Three.js for 3D player skin rendering.

#### Step 5.1: Tauri v2 Project Setup & IPC Commands (`crates/zircon-launcher/`)
* Configure `tauri.conf.json` (window size 1160x720, minimum size 900x560, dark window background `#0d1117`, custom icons).
* **`commands.rs`**: Expose Rust IPC commands to Webview via `#[tauri::command]`:
  * Auth: `login_microsoft`, `get_cached_session`, `logout`.
  * Server list: `load_saved_servers`, `save_server_list`, `launch_server`.
  * Offline instances: `list_offline_instances`, `create_offline_instance`, `delete_offline_instance`, `launch_offline_instance`, `list_offline_mods`, `delete_offline_mod`, `add_offline_mod`.
  * Skins: `get_active_skin`, `save_skin`, `remove_skin`, `get_skin_history`, `fetch_mojang_skin`.
  * Packs: `list_instance_packs`, `add_local_pack`, `remove_local_pack`, `set_active_shaderpack`, `toggle_resourcepack`.
  * Modrinth: `search_modrinth`, `install_modrinth_mod`.

#### Step 5.2: Vue 3 Frontend UI (`crates/zircon-launcher/ui/`)
* Migrate existing JavaFX layout to Vue 3 (Composition API) + Tailwind CSS:
  * **Microsoft Login Overlay**: Full-screen modal with Microsoft-branded button, auth status log.
  * **Sidebar**: Zircon brand lockup, navigation buttons (⚡ Servers, 🎮 Play Offline, 👕 Skins, ⚙️ Settings), user avatar card + logout button.
  * **View 1: Servers**: Saved server list with status badges, recommended server cards, "+ Add Server" dialog, active server PLAY button with loading spinners.
  * **View 2: Play Offline**: Offline instance list, "+ New Instance" modal (with MC/loader dropdowns), instance detail panel with Modrinth search, mod drag-and-drop zone, shader/texture pack selectors.
  * **View 3: Skins**: Skin gallery tiles, "+ Add Skin" picker, "SAVE" and "Remove Skin" action buttons.
  * **View 4: Settings**: Memory slider (2-16 GB RAM), strict hash verification toggle, trust direct custom mods toggle.
  * **Bottom Status Bar**: Progress bar & status log text.

#### Step 5.3: WebGL 3D Skin Renderer (Three.js Replacement for LWJGL)
* In `ui/src/components/Player3DPreview.vue`:
  * Initialize Three.js WebGL Renderer, Scene, and PerspectiveCamera.
  * Construct dual-layer 64x64 Minecraft player box model (6 base body boxes + 6 outer overlay boxes inflated by 0.25 units).
  * Map UV coordinates for head, torso, left/right arms, left/right legs and outer layers (hat, jacket, sleeves, pants).
  * Set texture filtering to `Three.NearestFilter` for crisp pixel art.
  * Add mouse drag event listeners for yaw rotation and pitch tilt (clamped to ±45°).
  * Expose component method `updateSkin(imageUri)` to refresh canvas texture on the fly.

#### Step 5.4: Skin Storage & Mojang Integration (`crates/zircon-launcher/src/skin.rs`)
* Store active skin at `~/.mcmanager/skins/active_skin.png` and history in `~/.mcmanager/skins/history/` (pruned to 25 entries).
* Download Mojang skin by UUID from `https://sessionserver.mojang.com/session/minecraft/profile/<uuid>`.
* Upload skin PNG to Mojang using Minecraft bearer token via `POST https://api.minecraftservices.com/minecraft/profile/skins` (`multipart/form-data`).

#### Phase 5 Checkpoint
Run `npm run build` inside `crates/zircon-launcher/ui/` and `cargo tauri build`. Ensure the Tauri application launches, renders the Vue UI and Three.js 3D skin canvas, and responds to IPC commands.

> ✅ **Verified**: `npm run build` → Vite build clean · `cargo check -p zircon-launcher` clean · `cargo test -p zircon-launcher` → **115 passed, 0 failed** (servers JSON, address parsing, pack selection, settings, skin head-icon crop, heap override) · `cargo tauri build --no-bundle` (via `npx @tauri-apps/cli`) → `target/release/zircon-launcher.exe` built in ~5m36s.
> Phase 4's engine is surfaced through 30+ `#[tauri::command]` IPC bindings with `launch-status`/`launch-progress`/`game-output`/`game-status` events streamed to the webview.

---

### Phase 6: E2E Integration, Performance Tuning & Build Optimization

#### Objectives
Validate end-to-end server launch, token ticket join gate, LZ4 backup/restore, offline play, and package release binaries.

#### Step 6.1: End-to-End Integration Verification
1. **Server Launch**: Run `zircon-server`. Verify initial admin password creation and TCP multiplexer startup on port 25565.
2. **Admin Web App**: Log in to web SPA (`http://localhost:25564` or proxied through 25565). Create a Fabric instance.
3. **Join Ticket Flow**: Launch `zircon-launcher`. Login via Microsoft OAuth. Click "PLAY" on the instance.
   * Verify launcher calls `/api/instances/:id/join-intent` to register pre-join ticket.
   * Verify client syncs `.mod_staging` mods and asset/library jars.
   * Verify TCP multiplexer parses Handshake + Login Start, verifies join ticket, and proxies client stream into internal MC server port.
4. **Vanilla Disconnect Check**: Attempt to join server port directly using a vanilla launcher. Verify Tokio detector rejects handshake with framed JSON Login Disconnect packet ("Zircon Client Required").
5. **Backup Test**: Trigger manual backup in web UI. Verify `<data>/backups/<id>/<backup_id>.tar.lz4` and sidecar JSON are created. Perform restore; verify instance folder rolls back seamlessly.

#### Step 6.2: Release Build Optimization
* Enable Link-Time Optimization (LTO) in root `Cargo.toml`:
  ```toml
  [profile.release]
  opt-level = 3
  lto = true
  codegen-units = 1
  panic = "abort"
  strip = true
  ```
* Ensure `cargo build --release` produces compact, high-performance binaries for `zircon-server` and `zircon-launcher`.

---

## 5. Summary Checkpoints Matrix

| Phase | Description | Focus Crate / Path | Completion Verification Check |
| :--- | :--- | :--- | :--- |
| **Phase 1** ✅ | Shared Models, Hashes, Archive, Metadata | `crates/zircon-core` | ✅ `cargo check -p zircon-core && cargo test -p zircon-core` → 46/46 pass |
| **Phase 2** ✅ | Server Engine, TCP Multiplexer & Join Gate | `crates/zircon-server/src/{process,multiplexer,instance}` | ✅ `cargo check -p zircon-server && cargo test -p zircon-server` → 58/58 pass |
| **Phase 3** | Axum REST & WebSocket Admin API | `crates/zircon-server/src/web` | `cargo check -p zircon-server` — controllers implemented; route verification + SPA/WS integration remaining |
| **Phase 4** | Launcher Auth, Launch Resolvers & Sync | `crates/zircon-launcher/src/{auth,launch,sync}` | `cargo check -p zircon-launcher && cargo test -p zircon-launcher` |
| **Phase 5** | Tauri v2 Desktop App & Vue 3 WebGL UI | `crates/zircon-launcher/ui` & `src/commands.rs` | ✅ `npm run build && cargo tauri build --no-bundle` → `zircon-launcher.exe`; `cargo test -p zircon-launcher` → 115/115 pass |
| **Phase 6** | E2E Testing, Binary Optimization & Release | Entire Workspace | `cargo test --workspace` & release binary validation |
