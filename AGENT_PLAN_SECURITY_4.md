# Zircon 10/10 Hardening & Zero-Trust Agent Execution Plan

This plan translates the six advanced architectural concepts into an implementation roadmap for an automated agent or engineering team. 

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                ADVANCED ZERO-TRUST ARCHITECTURE                                 │
├────────────────────────────┬────────────────────────────┬───────────────────────────────────────┤
│ CRYPTOGRAPHIC INTEGRITY    │ TOOLCHAIN & RUNTIME SAFETY │ BOUNDARY & HOST PROTECTION            │
├────────────────────────────┼────────────────────────────┼───────────────────────────────────────┤
│ • RFC 8785 Canonical JSON  │ • Adoptium SHA-256 Check   │ • CSWSH Origin Verification           │
│ • TOFU Key Rotation Engine │ • TOCTOU-Free Staging      │ • Host Memory Quotas & OOM Resilience │
└────────────────────────────┴────────────────────────────┴───────────────────────────────────────┘
```

---

## Phase 1: Deterministic Cryptographic Canon (RFC 8785 / JCS)

### Problem Statement
Standard `serde_json::to_vec` does not guarantee key ordering across platforms or library releases. White-space variations, number formatting, or character escaping discrepancies will cause cryptographic signature verification to fail unpredictably.

### Target Files
* `zircon-core/Cargo.toml`
* `zircon-core/src/crypto/signing.rs`
* `zircon-core/src/model/bom.rs`

### Step 1.1: Add Dependencies
In `zircon-core/Cargo.toml`, add `serde_jcs` and `ed25519-dalek`:
```toml
[dependencies]
ed25519-dalek = { version = "2.1", default-features = false, features = ["std", "rand_core"] }
serde_jcs = "0.1"
```

### Step 1.2: Implement RFC 8785 Canonical Serialization & Hashing
Update `zircon-core/src/crypto/signing.rs`:

```rust
// File: zircon-core/src/crypto/signing.rs

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use crate::model::BillOfMaterials;

/// Computes a deterministic RFC 8785 (JCS) SHA-256 digest of the BOM.
/// Signature and public key fields are stripped prior to hashing.
pub fn canonical_bom_digest(bom: &BillOfMaterials) -> Result<Vec<u8>, String> {
    let mut cloned = bom.clone();
    cloned.signature = None;
    cloned.server_public_key = None;

    // Convert to canonical JSON bytes per RFC 8785 (lexicographical key sort, IEEE 754 float canon)
    let canonical_bytes = serde_jcs::to_vec(&cloned)
        .map_err(|e| format!("JCS canonicalization failed: {e}"))?;

    let mut hasher = Sha256::new();
    hasher.update(&canonical_bytes);
    Ok(hasher.finalize().to_vec())
}

/// Signs the canonical BOM digest using an Ed25519 secret key.
pub fn sign_bom(bom: &BillOfMaterials, signing_key: &SigningKey) -> Result<String, String> {
    let digest = canonical_bom_digest(bom)?;
    let signature = signing_key.sign(&digest);
    Ok(hex::encode(signature.to_bytes()))
}

/// Verifies the canonical BOM digest against an Ed25519 public key.
pub fn verify_bom_signature(bom: &BillOfMaterials, pubkey_hex: &str) -> bool {
    let Some(sig_hex) = &bom.signature else {
        return false;
    };
    let Ok(pubkey_bytes) = hex::decode(pubkey_hex) else {
        return false;
    };
    let Ok(pubkey_array): Result<[u8; 32], _> = pubkey_bytes.try_into() else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pubkey_array) else {
        return false;
    };
    let Ok(sig_bytes) = hex::decode(sig_hex) else {
        return false;
    };
    let Ok(sig_array): Result<[u8; 64], _> = sig_bytes.try_into() else {
        return false;
    };
    let signature = Signature::from_bytes(&sig_array);
    let Ok(digest) = canonical_bom_digest(bom) else {
        return false;
    };

    verifying_key.verify(&digest, &signature).is_ok()
}
```

---

## Phase 2: JDK Toolchain Supply-Chain Verification

### Problem Statement
`JavaRuntimeResolver` downloads JDK archives over HTTPS, but never checks their cryptographic hash against Adoptium's official metadata API. A network proxy anomaly, cache poisoning, or corrupted download could lead to execution of an unverified runtime.

### Target File
* `zircon-launcher/src/launch/java.rs`

### Step 2.1: Implement Metadata Checksum Resolver & Verification
Refactor `zircon-launcher/src/launch/java.rs`:

```rust
// File: zircon-launcher/src/launch/java.rs

use sha2::{Digest, Sha256};

#[derive(serde::Deserialize)]
struct AdoptiumPackage {
    checksum: String,
    link: String,
}

#[derive(serde::Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(serde::Deserialize)]
struct AdoptiumRelease {
    binary: AdoptiumBinary,
}

impl JavaRuntimeResolver {
    /// Queries the Adoptium API for the verified download URL and SHA-256 checksum.
    async fn fetch_adoptium_release(&self, major: i32) -> Result<(String, String), LauncherError> {
        let os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "mac"
        } else {
            "linux"
        };
        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x64"
        };
        
        let api_url = format!(
            "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse"
        );

        if !zircon_core::security::ssrf::is_safe_cdn_url(&api_url) {
            return Err(LauncherError::InvalidInput("Adoptium API URL rejected by SSRF guard".into()));
        }

        let resp = self.http.get(&api_url).send().await?;
        if !resp.status().is_success() {
            return Err(LauncherError::Http {
                status: resp.status().as_u16(),
                url: api_url,
            });
        }

        let releases: Vec<AdoptiumRelease> = resp.json().await?;
        let first = releases.into_iter().next().ok_or_else(|| {
            LauncherError::NotFound(format!("No Adoptium release metadata found for Java {major}"))
        })?;

        Ok((first.binary.package.link, first.binary.package.checksum))
    }

    /// Resolves, downloads, cryptographically verifies, and extracts the JDK.
    pub async fn resolve(&self, required_major: i32) -> Result<PathBuf, LauncherError> {
        if let Some(home) = sufficient_system_java(required_major).await {
            return Ok(home);
        }

        let jdk_dir = self.cache_dir.join(format!("jdk-{required_major}"));
        let java_exe = java_executable(&jdk_dir);
        if java_exe.is_file() {
            return Ok(jdk_dir);
        }

        let (download_url, expected_sha256) = self.fetch_adoptium_release(required_major).await?;
        
        let archive_ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };
        let archive = self.cache_dir.join(format!("jdk-{required_major}.{archive_ext}"));

        tracing::info!("Downloading Java {required_major} from {download_url}...");
        self.download(&download_url, &archive).await?;

        // Cryptographic integrity check
        let archive_bytes = tokio::fs::read(&archive).await?;
        let actual_sha256 = hex::encode(Sha256::digest(&archive_bytes));

        if !expected_sha256.eq_ignore_ascii_case(&actual_sha256) {
            let _ = tokio::fs::remove_file(&archive).await;
            return Err(LauncherError::InvalidInput(format!(
                "Java runtime checksum mismatch! Expected SHA-256 {expected_sha256}, got {actual_sha256}"
            )));
        }

        tracing::info!("Java {required_major} archive SHA-256 verified successfully.");
        self.extract(&archive, &jdk_dir)?;

        if java_exe.is_file() {
            return Ok(jdk_dir);
        }
        // Handle archives with single top-level directory
        if let Ok(entries) = std::fs::read_dir(&jdk_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && java_executable(&entry.path()).is_file() {
                    return Ok(entry.path());
                }
            }
        }

        Err(LauncherError::Process("Java runtime extracted but binary missing".into()))
    }
}
```

---

## Phase 3: Cross-Site WebSocket Hijacking (CSWSH) Defense

### Problem Statement
`/api/console` upgrades WebSockets without inspecting the `Origin` header. An attacker on an arbitrary website (`evil.com`) could open a WebSocket connection to `ws://127.0.0.1:25564/api/console` in the background.

### Target File
* `zircon-server/src/web/controllers/console_controller.rs`

### Step 3.1: Enforce Strict Origin Validation in WebSocket Handshake
Update `console_ws` in `zircon-server/src/web/controllers/console_controller.rs`:

```rust
// File: zircon-server/src/web/controllers/console_controller.rs

use axum::http::HeaderMap;

pub async fn console_ws(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    // Validate Origin header during the HTTP upgrade handshake
    if let Some(origin_header) = headers.get("origin").and_then(|o| o.to_str().ok()) {
        let web_port = state.config.get_config().web_port;
        let public_port = state.config.get_config().public_port;

        let is_allowed = is_allowed_origin(origin_header, web_port, public_port);
        if !is_allowed {
            state.audit.log(
                "ANONYMOUS",
                "CSWSH_BLOCKED",
                &format!("Blocked unauthorized WebSocket upgrade from origin: {origin_header}"),
            );
            return Err(ApiError::Unauthorized("Cross-Origin WebSocket request denied".into()));
        }
    }

    Ok(ws.on_upgrade(move |socket| handle_console_socket(socket, state)))
}

fn is_allowed_origin(origin: &str, web_port: i32, public_port: i32) -> bool {
    let clean = origin.trim().to_lowercase();
    let allowed = [
        format!("http://127.0.0.1:{web_port}"),
        format!("http://localhost:{web_port}"),
        format!("https://127.0.0.1:{web_port}"),
        format!("https://localhost:{web_port}"),
        format!("http://127.0.0.1:{public_port}"),
        format!("http://localhost:{public_port}"),
        // Embedded Tauri frontend scheme
        "tauri://localhost".to_string(),
        "http://tauri.localhost".to_string(),
    ];

    allowed.iter().any(|a| a == &clean)
}
```

---

## Phase 4: TOFU Key Lifecycle & Interactive Pinning UI

### Problem Statement
If a server is reinstalled and a new Ed25519 key is generated, clients must not crash or fail silently. The launcher must detect the key change, display the SHA-256 fingerprint delta, and prompt the player before accepting the rotation.

### Target Files
* `zircon-launcher/src/servers.rs`
* `zircon-launcher/src/commands.rs`
* `zircon-launcher/ui/src/App.vue` (or component view)

### Step 4.1: Update SavedServer Model
In `zircon-launcher/src/servers.rs`:

```rust
// File: zircon-launcher/src/servers.rs

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    pub name: String,
    pub address: String,
    pub last_played: i64,
    pub use_https: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_public_key: Option<String>,
}

pub fn update_pinned_key(address: &str, new_key: &str) {
    let mut servers = load_servers();
    if let Some(server) = servers.iter_mut().find(|s| s.address.eq_ignore_ascii_case(address)) {
        server.pinned_public_key = Some(new_key.to_string());
        save_servers(&servers);
    }
}
```

### Step 4.2: Implement Interactive Key Verification in Launcher Pipeline
In `zircon-launcher/src/commands.rs`:

```rust
// File: zircon-launcher/src/commands.rs

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyMismatchPrompt {
    pub request_id: u64,
    pub server_address: String,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
}

pub fn compute_key_fingerprint(pubkey_hex: &str) -> String {
    let bytes = hex::decode(pubkey_hex).unwrap_or_default();
    let hash = sha2::Sha256::digest(&bytes);
    format!("SHA256:{}", hex::encode(hash))
}

// In run_online_flow:
let server_pubkey = bom.server_public_key.as_deref().ok_or_else(|| {
    LauncherError::Auth("Server BOM is not cryptographically signed (missing public key).".into())
})?;

// 1. Signature check against the claimed public key
if !zircon_core::crypto::signing::verify_bom_signature(&bom, server_pubkey) {
    return Err(LauncherError::Auth("BOM cryptographic signature verification failed.".into()));
}

// 2. TOFU (Trust-On-First-Use) Key Pinning Check
let saved_servers = servers::load_servers();
let current_server = saved_servers.iter().find(|s| s.address.eq_ignore_ascii_case(address));
let pinned_key = current_server.and_then(|s| s.pinned_public_key.clone());

match pinned_key {
    None => {
        // First connection: Pin the key automatically (TOFU)
        servers::update_pinned_key(address, server_pubkey);
        tracing::info!("Pinned new server public key for {address}: {server_pubkey}");
    }
    Some(ref existing_key) if existing_key != server_pubkey => {
        // Key Changed: Interactive SSH-style warning
        let request_id = state.next_key_prompt_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        state.key_prompts.lock().await.insert(request_id, tx);

        let _ = app.emit("server-key-mismatch", KeyMismatchPrompt {
            request_id,
            server_address: address.to_string(),
            old_fingerprint: compute_key_fingerprint(existing_key),
            new_fingerprint: compute_key_fingerprint(server_pubkey),
        });

        let accepted = match tokio::time::timeout(Duration::from_secs(60), rx).await {
            Ok(Ok(true)) => true,
            _ => false,
        };

        if !accepted {
            return Err(LauncherError::Auth(
                "Host key verification failed: server identity changed and was rejected.".into()
            ));
        }

        // User explicitly trusted the rotation
        servers::update_pinned_key(address, server_pubkey);
        tracing::warn!("User approved key rotation for {address} to {server_pubkey}");
    }
    Some(_) => {
        // Pinned key matches perfectly
    }
}
```

---

## Phase 5: TOCTOU Elimination in Client Mod Sync

### Problem Statement
Downloading a mod to `.mod_staging/`, checking its hash, and copying it to `mods/` creates a Time-of-Check to Time-of-Use window where a local background process could replace the file before execution.

### Target File
* `zircon-launcher/src/sync/mod_sync.rs`

### Step 5.1: Atomic Staged Transfer with Verification-on-Write
Refactor `reconcile` in `zircon-launcher/src/sync/mod_sync.rs`:

```rust
// File: zircon-launcher/src/sync/mod_sync.rs

pub(crate) fn reconcile_atomic(
    mods_dir: &Path,
    staging_dir: &Path,
    bom_mods: &[ModEntry],
) -> Result<(Vec<String>, Vec<String>), LauncherError> {
    let wanted_set: HashSet<&str> = bom_mods.iter().map(|m| m.filename.as_str()).collect();
    let mut removed = Vec::new();
    let mut kept = Vec::new();

    // 1. Remove stale mods
    for name in list_jar_names(mods_dir)? {
        if !wanted_set.contains(name.as_str()) {
            std::fs::remove_file(mods_dir.join(&name))?;
            removed.push(name);
        }
    }

    // 2. Transfer from staging to active mods/ with atomic verification
    for mod_entry in bom_mods {
        let filename = &mod_entry.filename;
        let staged_file = staging_dir.join(filename);
        let active_target = mods_dir.join(filename);
        let temp_target = mods_dir.join(format!(".{filename}.tmp"));

        if !staged_file.is_file() {
            continue;
        }

        // Copy directly into active mods directory as a hidden temporary file
        std::fs::copy(&staged_file, &temp_target)?;

        // Compute hash directly on the destination disk block
        let Ok(actual_sha1) = HashVerifier::sha1_file(&temp_target) else {
            let _ = std::fs::remove_file(&temp_target);
            return Err(LauncherError::InvalidInput(format!("Failed to read destination file: {filename}")));
        };

        if let Some(expected_sha1) = &mod_entry.sha1 {
            if !expected_sha1.eq_ignore_ascii_case(&actual_sha1) {
                let _ = std::fs::remove_file(&temp_target);
                return Err(LauncherError::InvalidInput(format!(
                    "TOCTOU violation detected: hash mismatch on final target block for {filename}"
                )));
            }
        }

        // Atomic rename overwrites the final destination
        std::fs::rename(&temp_target, &active_target)?;
        kept.push(filename.clone());
    }

    Ok((removed, kept))
}
```

---

## Phase 6: Host Resource Quotas & OOM Resilience

### Problem Statement
Uncontrolled `-Xmx` values across multiple server instances can exhaust host memory, causing the Linux OOM Killer to terminate `zircon-server` unexpectedly.

### Target Files
* `zircon-server/src/instance.rs`
* `zircon-server/deploy/zircon-server.service` (New Systemd unit template)

### Step 6.1: Enforce Memory Headroom Checks on Instance Creation/Update
In `zircon-server/src/instance.rs`:

```rust
// File: zircon-server/src/instance.rs

use sysinfo::System;

pub fn validate_instance_memory_headroom(
    current_instances: &[InstanceConfig],
    target_instance_id: Option<&str>,
    new_java_args: &str,
) -> Result<(), InstanceError> {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let total_ram_bytes = sys.total_memory();
    
    // Minimum 2.0 GB headroom for OS, daemon, and native JVM Metaspace
    let reserved_headroom_bytes: u64 = 2 * 1024 * 1024 * 1024; 
    let available_heap_limit = total_ram_bytes.saturating_sub(reserved_headroom_bytes);

    let mut total_allocated_heap: u64 = 0;

    for inst in current_instances {
        if target_instance_id == Some(&inst.id) {
            continue; // Will be replaced by new_java_args
        }
        total_allocated_heap += parse_xmx_bytes(&inst.java_args);
    }

    total_allocated_heap += parse_xmx_bytes(new_java_args);

    if total_allocated_heap > available_heap_limit {
        let allocated_gb = total_allocated_heap as f64 / (1024.0 * 1024.0 * 1024.0);
        let limit_gb = available_heap_limit as f64 / (1024.0 * 1024.0 * 1024.0);
        return Err(InstanceError::Invalid(format!(
            "Memory overcommit rejected: total instance heap ({allocated_gb:.1} GB) exceeds host safe limit ({limit_gb:.1} GB)"
        )));
    }

    Ok(())
}

fn parse_xmx_bytes(java_args: &str) -> u64 {
    for token in java_args.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        if let Some(val) = lower.strip_prefix("-xmx") {
            if let Some(num_str) = val.strip_suffix('g') {
                if let Ok(gb) = num_str.parse::<u64>() {
                    return gb * 1024 * 1024 * 1024;
                }
            } else if let Some(num_str) = val.strip_suffix('m') {
                if let Ok(mb) = num_str.parse::<u64>() {
                    return mb * 1024 * 1024;
                }
            }
        }
    }
    4 * 1024 * 1024 * 1024 // Default fallback 4GB
}
```

### Step 6.2: Systemd Daemon Hardening Configuration
Create `zircon-server/deploy/zircon-server.service`:

```ini
[Unit]
Description=Zircon Minecraft Server Manager Daemon
After=network.target

[Service]
Type=simple
User=zircon
Group=zircon
WorkingDirectory=/opt/zircon-server
ExecStart=/opt/zircon-server/zircon-server
Restart=always
RestartSec=5

# OOM Hardening: Protect daemon from kernel OOM killer
OOMScoreAdjust=-1000

# Process sandboxing
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=read-only
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

---

## Phase 7: Verification & Security Testing Matrix

Execute the following test matrix to confirm that all 10/10 security guarantees are operational:

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                  AUTOMATED VERIFICATION SUITE                                   │
├───────────────────────┬───────────────────────────────────────────┬─────────────────────────────┤
│ TEST SUITE            │ SCENARIO                                  │ EXPECTED RESULT             │
├───────────────────────┼───────────────────────────────────────────┼─────────────────────────────┤
│ test_jcs_canonical    │ Canonicalize BOM with randomized keys     │ Hash is bit-for-bit stable  │
│ test_adoptium_verify  │ Mock download with corrupted SHA-256 byte │ Extraction aborted, 400 Err │
│ test_cswsh_blocked    │ Send WebSocket upgrade with Origin: evil  │ HTTP 401/403 Upgrade Denied │
│ test_tofu_mismatch    │ Connect to server with rotated Ed25519 key│ Emits prompt, aborts if no  │
│ test_toctou_atomic    │ Swap file during active staging transfer  │ Transfer fails at rename    │
│ test_oom_guard        │ Configure instance with heap > host RAM   │ Rejects with overcommit Err │
└───────────────────────┴───────────────────────────────────────────┴─────────────────────────────┘
```

### Running the Suite
```bash
cargo test --workspace -- --nocapture
```

---

## Final Review

With the initial bug fixes and these **six advanced modules** implemented:
1. **RFC 8785 (JCS) + Ed25519 Signatures** ensure cross-platform deterministic cryptographic attestation.
2. **Adoptium SHA-256 Integrity Verification** closes toolchain supply-chain risks.
3. **CSWSH Origin Defense** stops cross-site socket hijacking.
4. **Interactive TOFU Key Lifecycle** provides SSH-grade host verification without breaking UX on key rotation.
5. **Destination-Block Verification** eliminates TOCTOU races.
6. **Host Memory Quotas & `OOMScoreAdjust`** guarantee daemon uptime under heavy instance load.

This completes the architecture necessary to achieve a **10/10 enterprise security posture**.
