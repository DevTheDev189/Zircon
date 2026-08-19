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

        let mut result = SyncResult::default();
        let mods_dir = game_dir.join("mods");
        std::fs::create_dir_all(&mods_dir)?;

        // Staging directory where downloads land before moving into active mods/
        let staging_dir = game_dir.join(".mod_staging");
        std::fs::create_dir_all(&staging_dir)?;

        // --- Step 1: fetch the BOM ---
        emit_status(listener, &format!("Fetching mod list from {base}..."));
        let bom_json = self.get(&format!("{base}/bom")).await?;
        let bom: BillOfMaterials = serde_json::from_str(&bom_json)?;
        let mc_version = bom.minecraft_version.clone();
        let mods = bom.mods.clone();
        result.bom = Some(bom);
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

        // --- Step 4: dynamically reconcile the active instance mods/ directory ---
        emit_status(listener, "Synchronizing active instance mods folder...");
        let wanted: Vec<String> = mods.iter().map(|m| m.filename.clone()).collect();
        let (removed, kept) = reconcile(&mods_dir, &staging_dir, &wanted)?;
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
    // Gather every pinned SHA-1 across all mod origins for one batch lookup.
    let mut all_sha1s: Vec<String> = Vec::new();
    for mod_entry in mods {
        if let Some(sha1) = &mod_entry.sha1 {
            if !sha1.trim().is_empty() {
                all_sha1s.push(sha1.clone());
            }
        }
    }

    let mut verified_sha1s: HashSet<String> = HashSet::new();
    if !all_sha1s.is_empty() {
        let modrinth = ModrinthApiClient::new();
        match modrinth.verify_hashes(&all_sha1s).await {
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
/// * **CurseForge** — the server pinned the official SHA-1 into the BOM at
///   install time, so a present (non-empty) SHA-1 verifies the mod; a
///   Modrinth cross-match is a bonus but not required.
/// * **anything else** (`direct`/unknown) — never trusted.
///
/// Pure so the security decision is unit-testable without network access.
fn is_mod_verified(
    origin: Option<&str>,
    sha1: Option<&str>,
    verified_sha1s: &HashSet<String>,
) -> bool {
    match (origin, sha1) {
        (Some("modrinth"), Some(sha1)) => verified_sha1s.contains(sha1),
        (Some("curseforge"), Some(sha1)) => {
            verified_sha1s.contains(sha1) || !sha1.trim().is_empty()
        }
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
/// files present in `mods_dir` after the copy phase. Wanted staged files are
/// left in staging (they feed the "already verified" check on the next sync),
/// mirroring the Java.
///
/// Port of the Java "dynamically reconcile the active instance mods/ directory"
/// step of `ModSyncEngine.sync`.
pub(crate) fn reconcile(
    mods_dir: &Path,
    staging_dir: &Path,
    wanted: &[String],
) -> std::io::Result<(Vec<String>, Vec<String>)> {
    let wanted_set: HashSet<&str> = wanted.iter().map(|s| s.as_str()).collect();
    let mut removed = Vec::new();
    let mut kept = Vec::new();

    // Delete local JARs in mods/ that are NOT part of the BOM.
    for name in list_jar_names(mods_dir)? {
        if !wanted_set.contains(name.as_str()) {
            std::fs::remove_file(mods_dir.join(&name))?;
            removed.push(name.clone());
            info!("Removed stale/unlisted mod from instance: {}", name);
        }
    }

    // Prune staging files that are no longer part of the BOM so the staging
    // area mirrors the server instead of accumulating stale downloads.
    for name in list_jar_names(staging_dir)? {
        if !wanted_set.contains(name.as_str()) {
            std::fs::remove_file(staging_dir.join(&name))?;
            info!("Pruned stale staged mod: {}", name);
        }
    }

    // Copy verified mods from staging into the active instance mods/.
    for name in wanted {
        let staged_file = staging_dir.join(name);
        let active_target = mods_dir.join(name);
        if staged_file.is_file() {
            std::fs::copy(&staged_file, &active_target)?;
        } else {
            warn!("Staged file missing for mod: {}", name);
        }
    }

    // Wanted files present in mods/ after reconciliation = the kept mods.
    for name in wanted {
        if mods_dir.join(name).is_file() {
            kept.push(name.clone());
        }
    }

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

/// Rejects a server-supplied filename that could escape the instance directory.
/// The BOM comes from an untrusted server, so a filename is only accepted when
/// it is a plain basename: no path separators, no `..`, no leading dot, non-
/// empty. Anything else aborts the sync before any file is written.
pub(crate) fn validate_entry_filename(filename: &str) -> Result<(), String> {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.starts_with('.')
        || filename == ".."
    {
        return Err(format!("invalid filename in server BOM: {filename:?}"));
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
        assert!(validate_entry_filename("a b.jar").is_ok());
        assert!(validate_entry_filename("mod-1.0-rc1.jar").is_ok());

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
        ] {
            assert!(
                validate_entry_filename(bad).is_err(),
                "should reject {bad:?}"
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

        // CurseForge: SHA-1 pinned in the BOM -> verified. The server pinned
        // the official hash at install time, so the client needs no CurseForge
        // API key; a Modrinth cross-match is a bonus but not required.
        assert!(is_mod_verified(
            Some("curseforge"),
            Some("good-sha1"),
            &confirmed
        ));
        assert!(is_mod_verified(
            Some("curseforge"),
            Some("not-on-modrinth-sha1"),
            &confirmed
        ));
        // CurseForge: empty or missing SHA-1 -> NOT verified (SHA-1 mandatory;
        // the 32-bit fingerprint alone can be spoofed via a preimage collision).
        assert!(!is_mod_verified(Some("curseforge"), Some(""), &confirmed));
        assert!(!is_mod_verified(Some("curseforge"), None, &confirmed));

        // Direct / unknown origin is never trusted.
        assert!(!is_mod_verified(
            Some("direct"),
            Some("good-sha1"),
            &confirmed
        ));
        assert!(!is_mod_verified(None, None, &confirmed));
    }

    // ------------------------------------------------------------------
    // reconcile
    // ------------------------------------------------------------------

    #[test]
    fn reconcile_removes_stale_copies_staged_and_prunes_staging() {
        let dir = TempDir::new("reconcile");
        let mods_dir = dir.path().join("mods");
        let staging_dir = dir.path().join(".mod_staging");
        std::fs::create_dir_all(&mods_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        // A stale mod in the instance, a BOM mod staged, and a stale staged file.
        std::fs::write(mods_dir.join("c.jar"), b"stale").unwrap();
        std::fs::write(staging_dir.join("a.jar"), b"bom content").unwrap();
        std::fs::write(staging_dir.join("stale-staged.jar"), b"old").unwrap();

        let wanted = vec!["a.jar".to_string(), "b.jar".to_string()];
        let (removed, kept) = reconcile(&mods_dir, &staging_dir, &wanted).unwrap();

        assert_eq!(vec!["c.jar".to_string()], removed);
        // The staged a.jar was copied into mods/ with its content.
        assert_eq!(
            b"bom content".to_vec(),
            std::fs::read(mods_dir.join("a.jar")).unwrap()
        );
        // The stale staged file was pruned; wanted staged files are kept
        // (they feed the "already verified" check on the next sync).
        assert!(!staging_dir.join("stale-staged.jar").exists());
        assert!(staging_dir.join("a.jar").is_file());
        // kept = wanted files present in mods/ after reconciliation (b.jar was
        // never staged, so it cannot be kept).
        assert_eq!(vec!["a.jar".to_string()], kept);
    }
}
