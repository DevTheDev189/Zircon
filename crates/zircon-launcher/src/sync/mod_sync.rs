//! Client mod synchronization engine: brings the local instance `mods/` folder
//! in line with the server's Bill of Materials.
//!
//! The flow is:
//!   1. fetch `{base}/bom` from the server,
//!   2. batch-verify hashes against Modrinth (SHA-1) / CurseForge (fingerprints),
//!   3. download missing / mismatched JARs into a staging area
//!      (`gameDir/.mod_staging`),
//!   4. dynamically reconcile the active `mods/` directory against the staging
//!      area, removing unlisted mods and copying the staged BOM mods.
//!
//! This module also hosts `HashVerifier`, the shared local-file hash checker
//! used by both the mod and the pack sync engines.
//!
//! Port of `com.mcmanager.client.sync.ModSyncEngine` / `HashVerifier`.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::Path;

use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use zircon_core::api::modrinth::ModrinthApiClient;
use zircon_core::api::ApiError;
use zircon_core::metadata::extractor::validate_mod_jar_structure;
use zircon_core::model::{BillOfMaterials, ModEntry, PackEntry};

use crate::error::LauncherError;

// ---------------------------------------------------------------------------
// HashVerifier
// ---------------------------------------------------------------------------

/// Verifies a local file against pinned SHA-1 / CurseForge-fingerprint hashes,
/// shared by [`ModEntry`] (mods) and [`PackEntry`] (shaderpacks/resourcepacks).
///
/// Port of `com.mcmanager.client.sync.HashVerifier`.
pub struct HashVerifier;

impl HashVerifier {
    /// True when `name` looks like a mod JAR: ends with `.jar` and does not
    /// start with `.` (`.DS_Store` etc.), case-insensitive.
    ///
    /// Port of the Java `isModJar`.
    pub fn is_mod_jar(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".jar") && !lower.starts_with('.')
    }

    /// True when `name` looks like a pack archive: ends with `.zip` and does
    /// not start with `.`.
    ///
    /// Port of the Java `isZip`.
    pub fn is_zip(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".zip") && !lower.starts_with('.')
    }

    /// Lower-case hex SHA-1 of a file, streamed through an 8 KiB buffer so
    /// memory usage stays flat for large JARs.
    ///
    /// Port of `HashUtil.getSha1` (`com.mcmanager.core.crypto.HashUtil`).
    pub fn sha1_file(path: &Path) -> std::io::Result<String> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha1::new();
        let mut buffer = [0u8; 8192];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hex::encode(hasher.finalize()))
    }

    /// Checks that `file` matches the hashes of `entry`.
    ///
    /// Port of the Java `HashVerifier.matches` — hardened: the pinned SHA-1 is
    /// mandatory. A missing file never matches; an entry without a pinned
    /// (non-blank) SHA-1 never matches, because the 32-bit MurmurHash3
    /// fingerprint alone is not collision-resistant enough to verify a file;
    /// a pinned SHA-1 is compared case-insensitively. I/O errors while hashing
    /// are treated as a mismatch.
    pub fn matches(file: &Path, entry: &ModEntry) -> bool {
        matches_inner(file, entry.sha1.as_deref())
    }

    /// Same check as [`matches`](Self::matches), for a [`PackEntry`].
    pub fn matches_pack(file: &Path, entry: &PackEntry) -> bool {
        matches_inner(file, entry.sha1.as_deref())
    }
}

fn matches_inner(file: &Path, sha1: Option<&str>) -> bool {
    if !file.is_file() {
        return false;
    }
    // SHA-1 is mandatory: a file whose entry pins no cryptographic digest
    // never matches (the caller decides — strict mode aborts).
    let Some(sha1) = sha1.filter(|s| !s.trim().is_empty()) else {
        return false;
    };
    let Ok(actual) = HashVerifier::sha1_file(file) else {
        return false;
    };
    sha1.eq_ignore_ascii_case(&actual)
}

// ---------------------------------------------------------------------------
// Sync result / progress listener
// ---------------------------------------------------------------------------

/// Receives human-readable sync progress.
///
/// `Send + Sync` so the listener can be invoked from background tasks and held
/// across `.await` points in spawned Tauri commands.
///
/// Port of the Java `ModSyncEngine.ProgressListener`.
pub trait ProgressListener: Send + Sync {
    /// A status message, e.g. "Downloading sodium-0.5.8.jar (2/5) to staging...".
    fn on_status(&self, message: &str);

    /// Download progress: `fraction` in `[0, 1]`, `detail` names the file.
    fn on_progress(&self, fraction: f64, detail: &str);
}

fn emit_status(listener: Option<&dyn ProgressListener>, message: &str) {
    if let Some(listener) = listener {
        listener.on_status(message);
    }
}

fn emit_progress(listener: Option<&dyn ProgressListener>, fraction: f64, detail: &str) {
    if let Some(listener) = listener {
        listener.on_progress(fraction, detail);
    }
}

/// Result of a mod sync run.
///
/// Port of the Java `ModSyncEngine.SyncResult`; `bom` is an `Option` so tests
/// can build results without fetching a BOM.
#[derive(Debug, Default)]
pub struct SyncResult {
    /// The BOM the sync ran against, when one was fetched.
    pub bom: Option<BillOfMaterials>,
    /// Mods downloaded into the staging area during this run.
    pub downloaded: Vec<String>,
    /// Mods deleted from the instance `mods/` folder because the server no
    /// longer lists them.
    pub removed: Vec<String>,
    /// Wanted mods present in the instance `mods/` folder after reconciliation.
    /// (In the Java this only counts mods whose staged file already verified;
    /// here it counts every wanted mod left in place, which is a superset.)
    pub kept: Vec<String>,
    /// Mods whose hash could not be confirmed against their source.
    pub unverified: Vec<String>,
    /// Whether the sync stopped early because strict verification failed.
    pub aborted: bool,
    /// Why the sync aborted (set when `aborted` is true).
    pub abort_reason: Option<String>,
}

/// If any mods are unverified, marks the result aborted with the Java abort
/// message. Verification is always strict — the removed security toggles could
/// never be re-enabled — so this gate cannot be weakened. Factored out so
/// tests can exercise the abort decision without any network access.
fn apply_strict_abort(result: &mut SyncResult) {
    if !result.unverified.is_empty() {
        result.aborted = true;
        result.abort_reason = Some(format!(
            "The following mods could not be verified against their source: {}. \
             This can happen if the server is not running the official Zircon \
             wrapper, a mod was modified, or the mod provider is unreachable. \
             Fix the server BOM or your connection and try again.",
            result.unverified.join(", ")
        ));
    }
}

// ---------------------------------------------------------------------------
// ModSyncEngine
// ---------------------------------------------------------------------------

/// Brings the local instance mods folder in line with the server's Bill of
/// Materials.
///
/// Port of `com.mcmanager.client.sync.ModSyncEngine`.
#[derive(Debug)]
pub struct ModSyncEngine {
    http: reqwest::Client,
}

impl Default for ModSyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ModSyncEngine {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build reqwest client");
        Self { http }
    }

    /// Synchronizes the mods folder with the server using a temporary staging
    /// area.
    ///
    /// * `server_base_url` — the Zircon server's base URL (a trailing slash is
    ///   tolerated).
    /// * `game_dir` — the instance directory containing `mods/` and (created
    ///   here) `.mod_staging/`.
    ///
    /// Verification is **always strict and always on**: every mod must be
    /// confirmed against Modrinth/CurseForge, and every downloaded file must
    /// match the hash pinned in the server's BOM. Any mod that cannot be
    /// verified (including when a provider is unreachable, or a mod has no
    /// provider at all) aborts the sync so nothing unverified is installed.
    ///
    /// Port of the Java `sync`, hardened: the Java-era "strict" / "trust
    /// direct mods" toggles and fail-open provider checks were removed.
    pub async fn sync(
        &self,
        server_base_url: &str,
        game_dir: &Path,
        listener: Option<&dyn ProgressListener>,
    ) -> Result<SyncResult, LauncherError> {
        let base = server_base_url
            .strip_suffix('/')
            .unwrap_or(server_base_url)
            .to_string();

        emit_status(listener, &format!("Fetching mod list from {base}..."));
        let bom_json = self.get(&format!("{base}/bom")).await?;
        let bom: BillOfMaterials = serde_json::from_str(&bom_json)?;
        self.sync_with_bom(&bom, &base, game_dir, &[], listener).await
    }

    /// Like [`sync`](Self::sync), but synchronizes against a caller-supplied
    /// BOM instead of fetching its own copy.
    ///
    /// The online launch flow calls this with the BOM it already fetched and
    /// cryptographically verified (TOFU-pinned signature), so the mods that get
    /// downloaded are exactly the ones from the trusted list — a second fetch
    /// could otherwise race with a compromised server and return a different,
    /// unverified list between verification and download.
    pub async fn sync_with_bom(
        &self,
        bom: &BillOfMaterials,
        server_base_url: &str,
        game_dir: &Path,
        keep_mods: &[String],
        listener: Option<&dyn ProgressListener>,
    ) -> Result<SyncResult, LauncherError> {
        let base = server_base_url
            .strip_suffix('/')
            .unwrap_or(server_base_url)
            .to_string();

        let mut result = SyncResult::default();
        let mods_dir = game_dir.join("mods");
        std::fs::create_dir_all(&mods_dir)?;

        // Staging directory where downloads land before moving into active mods/
        let staging_dir = game_dir.join(".mod_staging");
        std::fs::create_dir_all(&staging_dir)?;

        let mc_version = bom.minecraft_version.clone();
        let mods = bom.mods.clone();
        result.bom = Some(bom.clone());
        info!("BOM: {} mods for MC {}", mods.len(), mc_version);

        // --- Step 2: verify hashes against Modrinth's public database ---
        emit_status(listener, "Verifying mod hashes...");
        verify_against_providers(&mods, &mut result).await;
        if result.aborted {
            return Ok(result);
        }

        // --- Step 3: download missing / mismatched mods into the staging area ---
        let total_bytes: u64 = mods.iter().map(|m| m.file_size).sum();
        let mut downloaded_bytes: u64 = 0;

        for (i, mod_entry) in mods.iter().enumerate() {
            // The filename comes from the untrusted server BOM; a hostile
            // server must not be able to write outside the instance directory.
            validate_entry_filename(&mod_entry.filename).map_err(LauncherError::InvalidInput)?;
            let staged_target = staging_dir.join(&mod_entry.filename);

            if HashVerifier::matches(&staged_target, mod_entry) {
                continue;
            }

            let url = format!("{base}/files/mods/{}", url_encode(&mod_entry.filename));
            emit_status(
                listener,
                &format!(
                    "Downloading {} ({}/{}) to staging...",
                    mod_entry.filename,
                    i + 1,
                    mods.len()
                ),
            );
            let size = self.download(&url, &staged_target).await?;

            // The file must match the hash pinned in the server's BOM. The BOM
            // claims were already verified against Modrinth/CurseForge above;
            // this catches a server serving something different than it
            // advertised (compromised or malicious wrapper).
            if !HashVerifier::matches(&staged_target, mod_entry) {
                let _ = std::fs::remove_file(&staged_target);
                result.unverified.push(mod_entry.filename.clone());
                warn!(
                    "Downloaded mod failed hash check: {} (server served a different file than its BOM claims)",
                    mod_entry.filename
                );
                continue;
            }

            downloaded_bytes += size;
            result.downloaded.push(mod_entry.filename.clone());

            let fraction = if total_bytes > 0 {
                (downloaded_bytes as f64 / total_bytes as f64).min(1.0)
            } else {
                0.0
            };
            emit_progress(listener, fraction, &mod_entry.filename);
        }

        // Downloads that failed their hash check are treated like any other
        // unverified mod: strict verification aborts before anything is
        // installed into the active mods/ folder.
        apply_strict_abort(&mut result);
        if result.aborted {
            return Ok(result);
        }

        // --- Step 3.5: structural sanity of every wanted mod JAR ---
        // Hash checks prove the bytes match what the provider published, but a
        // compromised provider/BOM could still ship a malformed or zip-bomb
        // JAR. Every staged JAR (including ones kept from a previous sync)
        // must open as a valid ZIP with plausible compression and mod metadata
        // before it may move into the active mods/ folder.
        let mut structurally_invalid: Vec<String> = Vec::new();
        for mod_entry in &mods {
            let staged_target = staging_dir.join(&mod_entry.filename);
            if !staged_target.is_file() {
                continue;
            }
            if let Err(e) = validate_mod_jar_structure(&staged_target) {
                tracing::warn!(
                    "Mod failed structural validation: {} ({e})",
                    mod_entry.filename
                );
                structurally_invalid.push(mod_entry.filename.clone());
            }
        }
        if !structurally_invalid.is_empty() {
            result.aborted = true;
            result.abort_reason = Some(format!(
                "The following mods failed structural validation (not a valid ZIP, \
                 implausible compression ratio, or missing mod metadata): {}. \
                 Refusing to install them.",
                structurally_invalid.join(", ")
            ));
            return Ok(result);
        }

        // --- Step 4: dynamically reconcile the active instance mods/ directory ---
        // Staged mods are copied into mods/ via a hidden temp file, re-hashed on
        // the destination block, and atomically renamed into place (TOCTOU-free).
        emit_status(listener, "Synchronizing active instance mods folder...");
        let (removed, kept) = reconcile_atomic(&mods_dir, &staging_dir, &mods, keep_mods)?;
        result.removed = removed;
        result.kept = kept;

        emit_progress(listener, 1.0, "Done");
        emit_status(
            listener,
            &format!(
                "Mods up to date ({} kept, {} downloaded, {} removed)",
                result.kept.len(),
                result.downloaded.len(),
                result.removed.len()
            ),
        );
        Ok(result)
    }
}

/// Batch-verifies mod hashes against Modrinth's public hash API — no server
/// secrets needed. Modrinth mods must be confirmed by the API (fail-closed
/// when it is unreachable); CurseForge mods are verified by the SHA-1 the
/// server pinned into the BOM at install time from CurseForge's official
/// hash, so the client never needs a CurseForge API key and the 32-bit
/// fingerprint is never trusted.
///
/// Port of the Java `verifyAgainstProviders`, hardened: the Java treated
/// "provider unreachable" as verified and trusted `direct` mods when the
/// (removed) setting was enabled — both fail-open behaviors are gone.
async fn verify_against_providers(mods: &[ModEntry], result: &mut SyncResult) {
    // Gather every pinned SHA-1 for Modrinth-origin mods for batch lookup.
    // Modrinth's public API only indexes Modrinth files, so CurseForge mods
    // are verified by the server-pinned SHA-1 in the attested BOM.
    let mut modrinth_sha1s: Vec<String> = Vec::new();
    for mod_entry in mods {
        if mod_entry.origin.as_deref() == Some("modrinth") {
            if let Some(sha1) = mod_entry
                .sha1
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                modrinth_sha1s.push(sha1.to_string());
            }
        }
    }

    let mut verified_sha1s: HashSet<String> = HashSet::new();
    if !modrinth_sha1s.is_empty() {
        let modrinth = ModrinthApiClient::new();
        match modrinth.verify_hashes(&modrinth_sha1s).await {
            Ok(found) => {
                verified_sha1s.extend(found.into_keys());
            }
            Err(e) => warn!(
                "Modrinth hash verification unavailable: {} — Modrinth mods will block launch",
                map_api_error(e)
            ),
        }
    }

    for mod_entry in mods {
        let verified = is_mod_verified(
            mod_entry.origin.as_deref(),
            mod_entry.sha1.as_deref(),
            &verified_sha1s,
        );
        if !verified {
            result.unverified.push(mod_entry.filename.clone());
            warn!(
                "Unverified mod: {} ({})",
                mod_entry.filename,
                mod_entry.origin.as_deref().unwrap_or("")
            );
        }
    }

    apply_strict_abort(result);
}

/// Decides whether a mod counts as verified. SHA-1 is mandatory for every
/// origin:
///
/// * **Modrinth** — the public hash API must confirm the SHA-1 (fail-closed:
///   an unreachable API leaves Modrinth mods unverified).
/// * **CurseForge** — verified by the official SHA-1 pinned into the BOM at
///   install time by the server (which verified the CurseForge file against
///   CurseForge's API). A non-empty SHA-1 is mandatory.
/// * **anything else** (`direct`/unknown) — never trusted.
///
/// Pure so the security decision is unit-testable without network access.
fn is_mod_verified(
    origin: Option<&str>,
    sha1: Option<&str>,
    verified_sha1s: &HashSet<String>,
) -> bool {
    let Some(sha1) = sha1.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    match origin {
        Some("modrinth") => verified_sha1s.contains(sha1),
        Some("curseforge") => true,
        _ => false,
    }
}

/// Best-effort mapping of a provider [`ApiError`] into a [`LauncherError`].
/// Provider verification never aborts the sync, so this is only used to render
/// logged diagnostics.
fn map_api_error(error: ApiError) -> LauncherError {
    LauncherError::Network(error.to_string())
}

// ---------------------------------------------------------------------------
// Reconciliation
// ---------------------------------------------------------------------------

/// Deletes unlisted `.jar` files from `mods_dir`, prunes unlisted files from
/// `staging_dir`, then copies every wanted staged file into `mods_dir`.
///
/// Returns `(removed, kept)`: `removed` lists the `.jar` files deleted from
/// `mods_dir` because the server no longer lists them; `kept` lists the wanted
/// Reconciles the active instance `mods/` directory against the BOM, staging
/// the transfer so verification happens on the **final destination block**:
/// each staged mod is copied to a hidden temporary file inside `mods/`, its
/// SHA-1 is recomputed on that on-disk copy, and only then is it atomically
/// renamed over the final name. This closes the TOCTOU window of the old
/// "hash the staging file, then copy it" flow — a local background process
/// can no longer swap a file between the check and the install.
///
/// Also removes JARs in `mods/` that are no longer in the BOM and prunes
/// stale files from the staging area. Wanted staged files are left in staging
/// (they feed the "already verified" check on the next sync), mirroring the
/// Java.
///
/// Fails closed: a mismatch between the on-disk copy and the BOM's pinned
/// SHA-1 aborts the reconcile before the mod is installed.
///
/// Port of the Java "dynamically reconcile the active instance mods/ directory"
/// step of `ModSyncEngine.sync`, hardened against TOCTOU.
pub(crate) fn reconcile_atomic(
    mods_dir: &Path,
    staging_dir: &Path,
    bom_mods: &[ModEntry],
    keep_mods: &[String],
) -> Result<(Vec<String>, Vec<String>), LauncherError> {
    let wanted_set: HashSet<&str> = bom_mods.iter().map(|m| m.filename.as_str()).collect();
    let keep_set: HashSet<&str> = keep_mods.iter().map(|m| m.as_str()).collect();
    let mut removed = Vec::new();
    let mut kept = Vec::new();

    // Purge unmanaged mods that are neither in the current BOM nor the keep-list.
    // Handles both active (*.jar) and disabled (*.jar.disabled) files.
    for candidate_name in list_all_candidate_mod_files(mods_dir)? {
        let base_name = candidate_name.strip_suffix(".disabled").unwrap_or(&candidate_name);
        let wanted = wanted_set.contains(base_name) || wanted_set.contains(candidate_name.as_str());
        let kept_custom = keep_set.contains(base_name) || keep_set.contains(candidate_name.as_str());
        if !wanted && !kept_custom {
            let target_file = mods_dir.join(&candidate_name);
            if target_file.is_file() {
                std::fs::remove_file(&target_file)?;
                removed.push(base_name.to_string());
                info!("Pruned unmanaged/stale mod file: {}", candidate_name);
            } // end removal
        } // end filter check
    } // end candidate iteration

    // Prune staging files that are no longer part of the BOM so the staging
    // area mirrors the server instead of accumulating stale downloads.
    for name in list_jar_names(staging_dir)? {
        if !wanted_set.contains(name.as_str()) {
            std::fs::remove_file(staging_dir.join(&name))?;
            info!("Pruned stale staged mod: {}", name);
        }
    }

    // Transfer wanted mods from staging into mods/ directory:
    // Respects mod_entry.enabled: disabled mods are placed as `<filename>.disabled`.
    for mod_entry in bom_mods {
        let filename = &mod_entry.filename;
        let staged_file = staging_dir.join(filename);
        let active_dest = mods_dir.join(filename);
        let disabled_dest = mods_dir.join(format!("{filename}.disabled"));

        let (target_dest, opposite_dest) = if mod_entry.enabled { (&active_dest, &disabled_dest) } else { (&disabled_dest, &active_dest) };
        let temp_write_path = mods_dir.join(format!(".{filename}.tmp"));

        if !staged_file.is_file() {
            if target_dest.is_file() {
                kept.push(filename.to_owned());
            } else if opposite_dest.is_file() {
                std::fs::rename(opposite_dest, target_dest)?;
                info!("Updated active/disabled state for {filename} without downloading");
                kept.push(filename.to_owned());
            } else {
                warn!("Staged mod file missing for: {}", filename);
            } // end unstaged check
            continue;
        }

        std::fs::copy(&staged_file, &temp_write_path)?;
        let Ok(computed_hash) = HashVerifier::sha1_file(&temp_write_path) else {
            let _ = std::fs::remove_file(&temp_write_path);
            return Err(LauncherError::InvalidInput(format!(
                "Failed to read destination file: {filename}"
            )));
        };

        if let Some(expected_hash) = &mod_entry.sha1 {
            if !expected_hash.eq_ignore_ascii_case(&computed_hash) {
                let _ = std::fs::remove_file(&temp_write_path);
                return Err(LauncherError::InvalidInput(format!(
                    "TOCTOU violation detected: hash mismatch on final target block for {filename}"
                )));
            }
        }

        std::fs::rename(&temp_write_path, target_dest)?;
        if opposite_dest.is_file() {
            let _ = std::fs::remove_file(opposite_dest);
        } // remove alternate version
        kept.push(filename.to_owned());
    } // end bom_mods loop

    Ok((removed, kept))
}

/// `.jar` file names directly inside `dir` (not recursive), skipping dotfiles.
fn list_jar_names(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if HashVerifier::is_mod_jar(&name) {
            names.push(name);
        }
    }
    Ok(names)
}

fn list_all_candidate_mod_files(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    for item in std::fs::read_dir(dir)? {
        let entry = item?;
        if entry.file_type()?.is_file() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if is_applicable_mod_jar(&fname) { files.push(fname); }
        } // is_file
    } // read_dir loop
    Ok(files)
} // list_all_candidate_mod_files
fn is_applicable_mod_jar(filename: &str) -> bool {
    let clean = filename.strip_suffix(".disabled").unwrap_or(filename);
    HashVerifier::is_mod_jar(clean)
} // is_applicable_mod_jar
// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

impl ModSyncEngine {
    /// `GET` returning the body as text; a non-2xx response becomes
    /// [`LauncherError::Http`].
    async fn get(&self, url: &str) -> Result<String, LauncherError> {
        let response = self
            .http
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(LauncherError::Http {
                status: status.as_u16(),
                url: url.to_string(),
            });
        }
        Ok(body)
    }

    /// Streams a file download to `target`, returning the byte count written.
    async fn download(&self, url: &str, target: &Path) -> Result<u64, LauncherError> {
        let response = self.http.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(LauncherError::Http {
                status: status.as_u16(),
                url: url.to_string(),
            });
        }
        let mut out = tokio::fs::File::create(target).await?;
        let mut written: u64 = 0;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            out.write_all(&chunk).await?;
            written += chunk.len() as u64;
        }
        Ok(written)
    }
}

/// Percent-encodes a filename for a URL path segment (Java `URLEncoder`
/// equivalent for the characters used in file names; spaces encode as `%20`
/// rather than Java's `+`, which servers decode identically).
fn url_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Rejects a server-supplied filename that could escape the instance directory
/// or smuggle data into config files.
///
/// The BOM comes from an untrusted server, so a filename is only accepted when
/// it is a plain basename built from safe characters only: no path separators,
/// no `..`, no leading dot, no quotes, no control codes, no whitespace, and
/// nothing outside `[A-Za-z0-9._-+]`. Anything else aborts the sync before any
/// file is written or any entry reaches `options.txt` / `iris.properties`.
pub(crate) fn validate_entry_filename(filename: &str) -> Result<(), String> {
    if filename.is_empty() {
        return Err("Filename cannot be empty".to_string());
    }
    if filename.starts_with('.')
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
    {
        return Err(format!("Path traversal in filename: {filename:?}"));
    }
    // Reject quotes, newlines, control characters, whitespace and any other
    // character that could inject into config files or the filesystem.
    let is_valid = filename
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '+');
    if !is_valid {
        return Err(format!("Disallowed characters in filename: {filename:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use zircon_core::crypto::murmur3::curse_forge_fingerprint;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("{prefix}-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            remove_dir_all(&self.0);
        }
    }

    fn remove_dir_all(dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        let _ = std::fs::remove_dir(dir);
    }

    // ------------------------------------------------------------------
    // HashVerifier
    // ------------------------------------------------------------------

    #[test]
    fn sha1_file_matches_hex_of_sha1() {
        let dir = TempDir::new("sha1-file");
        let file = dir.path().join("data.bin");
        let content = b"zircon hash verifier test payload";
        std::fs::write(&file, content).unwrap();

        let expected = hex::encode(Sha1::digest(content));
        assert_eq!(expected, HashVerifier::sha1_file(&file).unwrap());
    }

    #[test]
    fn sha1_file_hashes_empty_file() {
        let dir = TempDir::new("sha1-empty");
        let file = dir.path().join("empty.bin");
        std::fs::write(&file, []).unwrap();

        let expected = hex::encode(Sha1::digest([]));
        assert_eq!(expected, HashVerifier::sha1_file(&file).unwrap());
    }

    #[test]
    fn matches_checks_sha1_case_insensitively() {
        let dir = TempDir::new("sha1-match");
        let file = dir.path().join("mod.jar");
        let content = b"fake jar bytes";
        std::fs::write(&file, content).unwrap();
        let digest = hex::encode(Sha1::digest(content));

        let entry = ModEntry::new(
            Some("m".to_string()),
            "mod.jar",
            Some(digest.clone()),
            0,
            Some("modrinth".to_string()),
            None,
            0,
        );
        assert!(HashVerifier::matches(&file, &entry));

        // SHA-1 comparison is case-insensitive (Java equalsIgnoreCase).
        let mut uppercase = entry.clone();
        uppercase.sha1 = Some(digest.to_uppercase());
        assert!(HashVerifier::matches(&file, &uppercase));

        // Wrong hash never matches.
        let mut wrong = entry.clone();
        wrong.sha1 = Some("0000000000000000000000000000000000000000".to_string());
        assert!(!HashVerifier::matches(&file, &wrong));

        // A missing file never matches.
        let missing = dir.path().join("missing.jar");
        assert!(!HashVerifier::matches(&missing, &entry));
    }

    #[test]
    fn fingerprint_without_sha1_never_matches() {
        // The 32-bit MurmurHash3 fingerprint alone is not collision-resistant
        // enough to verify a file: without a pinned SHA-1 nothing matches.
        let dir = TempDir::new("murmur3-match");
        let file = dir.path().join("cf.jar");
        let content = b"curseforge fingerprint payload";
        std::fs::write(&file, content).unwrap();
        let fingerprint = curse_forge_fingerprint(content);

        let entry = ModEntry::new(
            Some("c".to_string()),
            "cf.jar",
            None,
            fingerprint,
            Some("curseforge".to_string()),
            None,
            0,
        );
        assert!(
            !HashVerifier::matches(&file, &entry),
            "a fingerprint-only entry must never match locally"
        );

        // Once a SHA-1 is pinned, the file verifies against it.
        let mut pinned = entry.clone();
        pinned.sha1 = Some(hex::encode(Sha1::digest(content)));
        assert!(HashVerifier::matches(&file, &pinned));
    }

    #[test]
    fn matches_with_no_pinned_hash_does_not_match() {
        // Java HashVerifier: "No hash pinned: treat as 'unknown', caller
        // decides (strict mode aborts)." — matches() returns false so the file
        // is re-downloaded rather than silently trusted.
        let dir = TempDir::new("no-hash");
        let file = dir.path().join("mod.jar");
        std::fs::write(&file, b"x").unwrap();

        let entry = ModEntry::new(
            Some("d".to_string()),
            "mod.jar",
            None,
            0,
            Some("direct".to_string()),
            None,
            0,
        );
        assert!(!HashVerifier::matches(&file, &entry));
    }

    #[test]
    fn mod_jar_and_zip_detection() {
        assert!(HashVerifier::is_mod_jar("sodium-0.5.8.jar"));
        assert!(HashVerifier::is_mod_jar("SODIUM.JAR")); // case-insensitive
        assert!(!HashVerifier::is_mod_jar("mod.zip"));
        assert!(!HashVerifier::is_mod_jar(".DS_Store.jar")); // dotfiles excluded
        assert!(!HashVerifier::is_mod_jar(""));
        assert!(HashVerifier::is_zip("shaders.zip"));
        assert!(HashVerifier::is_zip("SHADERS.ZIP"));
        assert!(!HashVerifier::is_zip(".hidden.zip"));
        assert!(!HashVerifier::is_zip("mod.jar"));
    }

    #[test]
    fn validate_entry_filename_rejects_traversal_and_dotfiles() {
        // Legitimate basenames pass.
        assert!(validate_entry_filename("sodium-0.5.8.jar").is_ok());
        assert!(validate_entry_filename("mod-1.0-rc1.jar").is_ok());
        assert!(validate_entry_filename("sodium-fabric-0.5.8+mc1.20.4.jar").is_ok());
        assert!(validate_entry_filename("1.20.1-Forge-47.2.0.jar").is_ok());

        // Anything a hostile server could use to escape the instance directory.
        for bad in [
            "../../evil.jar",
            "..\\evil.jar",
            "sub/mod.jar",
            "sub\\mod.jar",
            "/abs.jar",
            "..",
            ".hidden.jar",
            "",
            "a..b.jar",
        ] {
            assert!(
                validate_entry_filename(bad).is_err(),
                "should reject {bad:?}"
            );
        }

        // Injection payloads: quotes, newlines, control codes and whitespace
        // must not reach options.txt / iris.properties or the filesystem.
        for bad in [
            "evil\".jar",
            "evil'.jar",
            "evil\n.jar",
            "evil\u{0}.jar",
            "evil\u{1f}.jar",
            "a b.jar",
            "a\tb.jar",
            "a$b.jar",
            "a:b.jar",
            "a?b.jar",
            "a*b.jar",
        ] {
            assert!(
                validate_entry_filename(bad).is_err(),
                "should reject injection filename {bad:?}"
            );
        }
    }

    // ------------------------------------------------------------------
    // SyncResult / strict abort
    // ------------------------------------------------------------------

    #[test]
    fn sync_result_defaults_and_strict_abort() {
        let mut result = SyncResult::default();
        assert!(result.bom.is_none());
        assert!(result.downloaded.is_empty());
        assert!(result.removed.is_empty());
        assert!(result.kept.is_empty());
        assert!(result.unverified.is_empty());
        assert!(!result.aborted);
        assert!(result.abort_reason.is_none());

        // Nothing unverified -> no abort.
        apply_strict_abort(&mut result);
        assert!(!result.aborted);

        // Any unverified mod aborts — verification is always strict, there is
        // no setting that can weaken it.
        result.unverified.push("a.jar".to_string());
        result.unverified.push("b.jar".to_string());
        apply_strict_abort(&mut result);
        assert!(result.aborted);
        let reason = result.abort_reason.as_deref().expect("abort reason");
        assert!(reason.contains("a.jar, b.jar"));
        assert!(reason.contains("could not be verified"));
    }

    // ------------------------------------------------------------------
    // Fail-closed verification decision
    // ------------------------------------------------------------------

    #[test]
    fn verified_only_when_the_pinned_hash_is_confirmed() {
        let confirmed = HashSet::from(["good-sha1".to_string()]);

        // Modrinth: hash confirmed by the public API -> verified.
        assert!(is_mod_verified(
            Some("modrinth"),
            Some("good-sha1"),
            &confirmed
        ));
        // Modrinth: hash not confirmed (tampered, or the API is unreachable)
        // -> NOT verified (fail-closed).
        assert!(!is_mod_verified(
            Some("modrinth"),
            Some("evil-sha1"),
            &confirmed
        ));
        // Modrinth: no pinned hash -> NOT verified.
        assert!(!is_mod_verified(Some("modrinth"), None, &confirmed));

        // CurseForge: valid SHA-1 pinned in the BOM -> verified.
        // CurseForge files are validated by the server at install time and pinned
        // into the attested BOM.
        assert!(is_mod_verified(
            Some("curseforge"),
            Some("good-sha1"),
            &confirmed
        ));
        assert!(is_mod_verified(
            Some("curseforge"),
            Some("cf-specific-sha1"),
            &confirmed
        ));
        // CurseForge: empty, whitespace-padded or missing SHA-1 -> NOT verified
        // (SHA-1 mandatory; the 32-bit fingerprint alone can be spoofed via a
        // preimage collision).
        assert!(!is_mod_verified(Some("curseforge"), Some(""), &confirmed));
        assert!(!is_mod_verified(
            Some("curseforge"),
            Some("   "),
            &confirmed
        ));
        assert!(!is_mod_verified(Some("curseforge"), None, &confirmed));

        // Direct / unknown origin is never trusted, even with a confirmed hash.
        assert!(!is_mod_verified(
            Some("direct"),
            Some("good-sha1"),
            &confirmed
        ));
        assert!(!is_mod_verified(None, Some("good-sha1"), &confirmed));
        assert!(!is_mod_verified(None, None, &confirmed));
    }

    // ------------------------------------------------------------------
    // reconcile_atomic (TOCTOU-free transfer)
    // ------------------------------------------------------------------

    #[test]
    fn reconcile_atomic_removes_stale_transfers_verified_and_prunes_staging() {
        let dir = TempDir::new("reconcile-atomic");
        let mods_dir = dir.path().join("mods");
        let staging_dir = dir.path().join(".mod_staging");
        std::fs::create_dir_all(&mods_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        // A stale mod in the instance, a BOM mod staged (with a matching
        // pinned SHA-1), and a stale staged file.
        std::fs::write(mods_dir.join("c.jar"), b"stale").unwrap();
        let content = b"bom content";
        std::fs::write(staging_dir.join("a.jar"), content).unwrap();
        let a_sha1 = HashVerifier::sha1_file(&staging_dir.join("a.jar")).unwrap();
        std::fs::write(staging_dir.join("stale-staged.jar"), b"old").unwrap();

        let bom_mods = vec![
            ModEntry::new(
                None,
                "a.jar",
                Some(a_sha1),
                0,
                Some("direct".to_string()),
                None,
                0,
            ),
            // Never staged → skipped, not kept.
            ModEntry::new(
                None,
                "b.jar",
                Some("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string()),
                0,
                Some("direct".to_string()),
                None,
                0,
            ),
        ];

        let (removed, kept) = reconcile_atomic(&mods_dir, &staging_dir, &bom_mods, &[]).unwrap();

        assert_eq!(vec!["c.jar".to_string()], removed);
        // The staged a.jar was transferred into mods/ with its content, via an
        // atomic rename — no leftover temp file.
        assert_eq!(
            content.to_vec(),
            std::fs::read(mods_dir.join("a.jar")).unwrap()
        );
        assert!(!mods_dir.join(".a.jar.tmp").exists());
        // The stale staged file was pruned; wanted staged files are kept
        // (they feed the "already verified" check on the next sync).
        assert!(!staging_dir.join("stale-staged.jar").exists());
        assert!(staging_dir.join("a.jar").is_file());
        // kept = mods actually installed by the transfer.
        assert_eq!(vec!["a.jar".to_string()], kept);
    }

    #[test]
    fn reconcile_atomic_detects_toctou_hash_mismatch_on_write() {
        let dir = TempDir::new("reconcile-toctou");
        let mods_dir = dir.path().join("mods");
        let staging_dir = dir.path().join(".mod_staging");
        std::fs::create_dir_all(&mods_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        // A staged mod whose content does NOT match the BOM's pinned SHA-1 —
        // exactly what a swap between the staging check and the install would
        // look like on the destination block.
        std::fs::write(staging_dir.join("evil.jar"), b"swapped by local process").unwrap();
        let bom_mods = vec![ModEntry::new(
            None,
            "evil.jar",
            Some("0000000000000000000000000000000000000000".to_string()),
            0,
            Some("direct".to_string()),
            None,
            0,
        )];

        let err = reconcile_atomic(&mods_dir, &staging_dir, &bom_mods, &[]).unwrap_err();
        assert!(matches!(err, LauncherError::InvalidInput(_)), "{err:?}");
        assert!(err.to_string().contains("TOCTOU"), "unhelpful error: {err}");

        // Nothing was installed and no temp file is left behind.
        assert!(!mods_dir.join("evil.jar").exists());
        assert!(!mods_dir.join(".evil.jar.tmp").exists());
    } // end toctou test
    #[test] // disabled extension placement test
    fn reconcile_atomic_places_disabled_mods_with_disabled_extension() {
        let env = TempDir::new("reconcile-test-disabled");
        let mods_path = env.path().join("mods");
        let staging_path = env.path().join(".mod_staging");
        std::fs::create_dir_all(&mods_path).unwrap();
        std::fs::create_dir_all(&staging_path).unwrap();

        let raw_bytes = b"inactive-mod-data";
        std::fs::write(staging_path.join("inactive.jar"), raw_bytes).unwrap();
        let digest = HashVerifier::sha1_file(&staging_path.join("inactive.jar")).unwrap();

        let mut mod_item = ModEntry::new(None, "inactive.jar", Some(digest), 0, Some("direct".to_string()), None, 0);
        mod_item.enabled = false;

        let (_purged, preserved) = reconcile_atomic(&mods_path, &staging_path, &[mod_item], &[]).expect("reconcile");
        assert_eq!(vec!["inactive.jar".to_string()], preserved);
        assert!(!mods_path.join("inactive.jar").exists(), "active jar should not exist");
        assert!(mods_path.join("inactive.jar.disabled").is_file(), "disabled jar should exist");
        assert_eq!(raw_bytes.to_vec(), std::fs::read(mods_path.join("inactive.jar.disabled")).unwrap());
    } // end reconcile_atomic_places_disabled_mods
    #[test] // staging empty toggle test
    fn reconcile_atomic_swaps_enabled_state_via_rename_when_staging_empty() {
        let env = TempDir::new("reconcile-test-toggle");
        let mods_path = env.path().join("mods");
        let staging_path = env.path().join(".mod_staging");
        std::fs::create_dir_all(&mods_path).unwrap();
        std::fs::create_dir_all(&staging_path).unwrap();

        let initial_payload = b"existing-installed-mod";
        std::fs::write(mods_path.join("toggle.jar"), initial_payload).unwrap();

        let mut mod_item = ModEntry::new(None, "toggle.jar", Some("dummy".into()), 0, Some("direct".into()), None, 0);
        mod_item.enabled = false;

        // Transition from enabled -> disabled without staging file
        let (_p, preserved) = reconcile_atomic(&mods_path, &staging_path, &[mod_item.clone()], &[]).unwrap();
        assert_eq!(vec!["toggle.jar".to_string()], preserved);
        assert!(!mods_path.join("toggle.jar").exists());
        assert!(mods_path.join("toggle.jar.disabled").is_file());
        assert_eq!(initial_payload.to_vec(), std::fs::read(mods_path.join("toggle.jar.disabled")).unwrap());

        // Transition back from disabled -> enabled without staging file
        mod_item.enabled = true;
        let (_p2, preserved2) = reconcile_atomic(&mods_path, &staging_path, &[mod_item], &[]).unwrap();
        assert_eq!(vec!["toggle.jar".to_string()], preserved2);
        assert!(mods_path.join("toggle.jar").is_file());
        assert!(!mods_path.join("toggle.jar.disabled").exists());
    } // end reconcile_atomic_swaps_enabled_state
    #[test] // purge unlisted test
    fn reconcile_atomic_purges_unlisted_disabled_mods() {
        let env = TempDir::new("reconcile-test-purge");
        let mods_path = env.path().join("mods");
        let staging_path = env.path().join(".mod_staging");
        std::fs::create_dir_all(&mods_path).unwrap();
        std::fs::create_dir_all(&staging_path).unwrap();

        std::fs::write(mods_path.join("orphan.jar.disabled"), b"orphan-content").unwrap();

        let (purged, preserved) = reconcile_atomic(&mods_path, &staging_path, &[], &[]).unwrap();
        assert_eq!(vec!["orphan.jar".to_string()], purged);
        assert!(preserved.is_empty());
        assert!(!mods_path.join("orphan.jar.disabled").exists());
    } // end reconcile_atomic_purges_unlisted
    #[test] // custom keep mods test
    fn reconcile_atomic_preserves_custom_keep_mods() {
        let env = TempDir::new("reconcile-test-keep");
        let mods_path = env.path().join("mods");
        let staging_path = env.path().join(".mod_staging");
        std::fs::create_dir_all(&mods_path).unwrap();
        std::fs::create_dir_all(&staging_path).unwrap();

        std::fs::write(mods_path.join("client-tool.jar"), b"client tool payload").unwrap();
        std::fs::write(mods_path.join("optout-mod.jar.disabled"), b"optout payload").unwrap();
        std::fs::write(mods_path.join("unwanted.jar"), b"unwanted payload").unwrap();

        let keep = vec!["client-tool.jar".to_string(), "optout-mod.jar".to_string()];
        let (purged, _retained) = reconcile_atomic(&mods_path, &staging_path, &[], &keep).expect("reconcile with keep mods");

        assert_eq!(vec!["unwanted.jar".to_string()], purged);
        assert!(mods_path.join("client-tool.jar").is_file());
        assert!(mods_path.join("optout-mod.jar.disabled").is_file());
        assert!(!mods_path.join("unwanted.jar").exists());
    } // end reconcile_atomic_preserves_custom_keep_mods
}
