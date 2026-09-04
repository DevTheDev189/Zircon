//! Server-side crash log analyzer and 1-click automated remediation service.
//!
//! When a Minecraft server instance fails or terminates unexpectedly, this
//! service scans `crash-reports/` and `logs/latest.log` to identify root causes,
//! queries the Modrinth / CurseForge APIs to pre-resolve missing or incompatible
//! dependencies, and provides actionable 1-click remediation payloads.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::services::bom::BomService;
use crate::services::mods::ModManagementService;

/// High-level diagnostic classification of a crash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CrashCategory {
    MissingDependency,
    OutdatedDependency,
    ModIncompatibility,
    DuplicateMods,
    JavaVersionMismatch,
    OutOfMemory,
    CorruptedConfig,
    CorruptedWorld,
    MixinFailure,
    ClientOnlyModOnServer,
    EulaNotAccepted,
    GenericCrashReport,
    Unknown,
}

/// Action payload for 1-click automated remediation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "actionType", content = "payload", rename_all = "camelCase")]
pub enum CrashRemediationAction {
    InstallMissingMod {
        provider: String,
        project_id: String,
        project_name: String,
        file_name: String,
        download_url: String,
        version_number: String,
    },
    UpdateMod {
        provider: String,
        old_file_name: String,
        new_file_name: String,
        download_url: String,
        version_number: String,
    },
    DisableMod {
        file_name: String,
        mod_name: String,
    },
    RemoveDuplicateMod {
        file_name: String,
        keep_file_name: String,
    },
    AdjustMemory {
        current_gb: u32,
        recommended_gb: u32,
    },
    ResetConfigFile {
        file_path: String,
        backup_path: String,
    },
    ChangeJavaVersion {
        required_major: i32,
    },
    AcceptEula,
    OpenHelpUrl {
        title: String,
        url: String,
    },
}

/// Detailed crash diagnostic report returned to the admin dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashAnalysis {
    pub is_crash: bool,
    pub category: CrashCategory,
    pub title: String,
    pub plain_english_summary: String,
    pub technical_details: String,
    pub relevant_log_lines: Vec<String>,
    pub primary_action: Option<CrashRemediationAction>,
    pub secondary_actions: Vec<CrashRemediationAction>,
    pub sanitized_log: String,
}

impl CrashAnalysis {
    pub fn healthy(summary: &str) -> Self {
        Self {
            is_crash: false,
            category: CrashCategory::Unknown,
            title: "No Crashes Detected".into(),
            plain_english_summary: summary.into(),
            technical_details: String::new(),
            relevant_log_lines: Vec::new(),
            primary_action: None,
            secondary_actions: Vec::new(),
            sanitized_log: String::new(),
        }
    }
}

/// Parses crash logs, performs heuristic matching, and resolves API fixes.
pub struct CrashAnalyzerService;

impl CrashAnalyzerService {
    /// Evaluates an instance directory for crashes, returning a complete analysis.
    pub async fn analyze_instance(
        instance_dir: &Path,
        mc_version: &str,
        loader_type: &str,
        mod_service: Option<&ModManagementService>,
        session_start: Option<std::time::SystemTime>,
    ) -> CrashAnalysis {
        let (raw_content, source_name) = match Self::find_latest_crash_content(instance_dir, session_start) {
            Some(res) => res,
            None => {
                return CrashAnalysis::healthy("The server appears healthy and no crash logs were found.");
            }
        };

        let sanitized_content = sanitize_log_text(&raw_content);
        let mut analysis = Self::diagnose_heuristics(&sanitized_content, &source_name);

        // If a missing or outdated dependency was diagnosed, try to pre-resolve via Modrinth/CurseForge
        if let Some(mod_svc) = mod_service {
            Self::enrich_with_api_resolution(
                &mut analysis,
                instance_dir,
                mc_version,
                loader_type,
                mod_svc,
            )
            .await;
        }

        analysis.sanitized_log = sanitized_content;
        analysis
    }

    /// Finds the newest crash report or latest.log in the instance directory or its server subdirectory.
    /// Uses `session_start` to discard stale crash reports or logs from prior runs.
    fn find_latest_crash_content(
        instance_dir: &Path,
        session_start: Option<std::time::SystemTime>,
    ) -> Option<(String, String)> {
        let candidates = [
            instance_dir.join("server"),
            instance_dir.to_path_buf(),
        ];

        // 1. Look for genuine crash reports in crash-reports/
        for base in &candidates {
            let crash_dir = base.join("crash-reports");
            if crash_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&crash_dir) {
                    let mut files: Vec<(PathBuf, std::time::SystemTime)> = entries
                        .flatten()
                        .filter_map(|e| {
                            let path = e.path();
                            if path.is_file() {
                                let modified = e.metadata().ok().and_then(|m| m.modified().ok())?;
                                Some((path, modified))
                            } else {
                                None
                            }
                        })
                        .collect();

                    // Sort newest modified first
                    files.sort_by(|a, b| b.1.cmp(&a.1));

                    for (latest, modified) in files {
                        // If we have a session start time, ignore crash reports older than start time (with 30s grace)
                        if let Some(start) = session_start {
                            let cutoff = start.checked_sub(std::time::Duration::from_secs(30)).unwrap_or(start);
                            if modified < cutoff {
                                continue;
                            }
                        } else if let Ok(elapsed) = modified.elapsed() {
                            // If no session start time, ignore crash reports older than 2 hours
                            if elapsed > std::time::Duration::from_secs(7200) {
                                continue;
                            }
                        }

                        if let Ok(content) = fs::read_to_string(&latest) {
                            if !content.trim().is_empty() {
                                let name = latest.file_name().unwrap_or_default().to_string_lossy().to_string();
                                return Some((content, name));
                            }
                        }
                    }
                }
            }
        }

        // 2. Look for latest.log
        for base in &candidates {
            let log_file = base.join("logs").join("latest.log");
            if log_file.is_file() {
                if let Some(start) = session_start {
                    if let Ok(meta) = log_file.metadata() {
                        if let Ok(mod_time) = meta.modified() {
                            let cutoff = start.checked_sub(std::time::Duration::from_secs(30)).unwrap_or(start);
                            if mod_time < cutoff {
                                continue;
                            }
                        }
                    }
                }

                if let Ok(content) = fs::read_to_string(&log_file) {
                    if !content.trim().is_empty() {
                        // Focus on tail (last 300 lines) of latest.log to examine the actual crash/shutdown
                        let lines: Vec<&str> = content.lines().collect();
                        let tail_lines = if lines.len() > 300 {
                            &lines[lines.len() - 300..]
                        } else {
                            &lines[..]
                        };
                        let tail_content = tail_lines.join("\n");
                        return Some((tail_content, "logs/latest.log".to_string()));
                    }
                }
            }
        }

        None
    }

    /// Performs heuristic pattern matching on sanitized log content.
    pub fn diagnose_heuristics(log_text: &str, source_name: &str) -> CrashAnalysis {
        let lines: Vec<&str> = log_text.lines().collect();

        // 1. Missing Dependencies (Fabric/Quilt & Forge/NeoForge)
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("ModResolutionException")
                || line.contains("ModLoadingIssue")
                || line.contains("MissingMandatoryDependenciesException")
                || line.contains("Missing or unsupported mandatory dependencies")
                || line.contains("A potential solution has been determined")
                || (line.contains("requires") && (line.contains("which is missing") || line.contains("of mod") || line.contains("or above") || line.contains("any version of")))
            {
                let snippet = extract_snippet(&lines, i, 8);
                let missing_slug = extract_missing_mod_identifier(line, &snippet);
                let title = format!(
                    "Missing Mod Dependency: {}",
                    missing_slug.as_deref().unwrap_or("Required Library")
                );
                let summary = if let Some(ref slug) = missing_slug {
                    format!(
                        "The server failed to start because the mod '{slug}' (or one of its required dependencies) is missing from the mods folder."
                    )
                } else {
                    "The server is missing a required mod dependency to start.".into()
                };

                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::MissingDependency,
                    title,
                    plain_english_summary: summary,
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: snippet,
                    primary_action: None, // Filled during API resolution
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 2. Duplicate Mod Files
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("DuplicateModsFoundException")
                || line.contains("Found duplicate mods")
                || line.contains("Duplicate mod ID")
            {
                let snippet = extract_snippet(&lines, i, 6);
                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::DuplicateMods,
                    title: "Duplicate Mod Files Detected".into(),
                    plain_english_summary: "Multiple versions of the same mod were found in the mods directory.".into(),
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: snippet,
                    primary_action: None,
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 3. Outdated / Incompatible Mod Version
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("IncompatibleModException")
                || line.contains("Incompatible mods found")
                || (line.contains("requires version") && line.contains("but only"))
                || line.contains("is incompatible with")
            {
                let snippet = extract_snippet(&lines, i, 8);
                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::ModIncompatibility,
                    title: "Mod Incompatibility Detected".into(),
                    plain_english_summary: "One or more installed mods are incompatible with each other or require updated versions.".into(),
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: snippet,
                    primary_action: None,
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 4. Java Version Mismatch
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("UnsupportedClassVersionError")
                || line.contains("has been compiled by a more recent version of the Java Runtime")
            {
                let req_major = extract_required_java_major(line);
                let title = format!("Java Runtime Mismatch (Requires Java {req_major})");
                let summary = format!(
                    "A mod or server jar was compiled for Java {req_major}, but the server is running an older Java version."
                );

                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::JavaVersionMismatch,
                    title,
                    plain_english_summary: summary,
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: extract_snippet(&lines, i, 4),
                    primary_action: Some(CrashRemediationAction::ChangeJavaVersion {
                        required_major: req_major,
                    }),
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 5. Out Of Memory (OOM)
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("java.lang.OutOfMemoryError")
                || line.contains("GC overhead limit exceeded")
                || line.contains("Java heap space")
            {
                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::OutOfMemory,
                    title: "Server Out of Memory (Heap Exhausted)".into(),
                    plain_english_summary: "The Minecraft server exhausted its allocated RAM heap during startup or tick loop.".into(),
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: extract_snippet(&lines, i, 4),
                    primary_action: Some(CrashRemediationAction::AdjustMemory {
                        current_gb: 4,
                        recommended_gb: 6,
                    }),
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 6. Corrupted Configuration / JSON / TOML
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("JsonSyntaxException")
                || line.contains("TomlDecodingException")
                || line.contains("MalformedJsonException")
                || line.contains("Failed to parse config")
            {
                let file_path = extract_config_file_path(line);
                let snippet = extract_snippet(&lines, i, 6);
                let title = "Corrupted Mod Configuration File".into();
                let summary = format!(
                    "A configuration file ({}) contains invalid syntax and crashed the server.",
                    file_path.as_deref().unwrap_or("config file")
                );

                let primary = file_path.map(|fp| CrashRemediationAction::ResetConfigFile {
                    file_path: fp.clone(),
                    backup_path: format!("{fp}.bak"),
                });

                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::CorruptedConfig,
                    title,
                    plain_english_summary: summary,
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: snippet,
                    primary_action: primary,
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 7. Client-Only Mod executed on Dedicated Server
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("java.lang.NoClassDefFoundError: net/minecraft/client")
                || line.contains("Attempted to load class net/minecraft/client")
                || line.contains("@Environment(EnvType.CLIENT)")
            {
                let snippet = extract_snippet(&lines, i, 8);
                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::ClientOnlyModOnServer,
                    title: "Client-Only Mod Crashed Server".into(),
                    plain_english_summary: "A client-only mod (such as a GUI, minimap, or shader mod) was loaded on the dedicated server and attempted to reference client graphics classes.".into(),
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: snippet,
                    primary_action: None,
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 8. EULA not accepted
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("You need to agree to the EULA in order to run the server")
                || line.contains("Failed to load eula.txt")
            {
                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::EulaNotAccepted,
                    title: "Minecraft EULA Not Accepted".into(),
                    plain_english_summary: "The server stopped because the Minecraft EULA has not been accepted.".into(),
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: extract_snippet(&lines, i, 4),
                    primary_action: Some(CrashRemediationAction::AcceptEula),
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 9. Corrupted World
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("Failed to load level.dat")
                || line.contains("Corrupted world")
                || line.contains("Encountered an unexpected exception while loading world")
            {
                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::CorruptedWorld,
                    title: "Corrupted World Data (level.dat)".into(),
                    plain_english_summary: "The server world data is damaged or cannot be parsed.".into(),
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: extract_snippet(&lines, i, 6),
                    primary_action: None,
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 10. Mixin / Bytecode Injection Failures
        for (i, &line) in lines.iter().enumerate() {
            if line.contains("MixinTransformerError")
                || line.contains("Critical injection failure")
                || line.contains("MixinApplyError")
            {
                let snippet = extract_snippet(&lines, i, 8);
                return CrashAnalysis {
                    is_crash: true,
                    category: CrashCategory::MixinFailure,
                    title: "Bytecode Injection Failure (Mixin Conflict)".into(),
                    plain_english_summary: "A mod failed to patch the server code, likely due to a conflicting mod or incompatible Minecraft minor version.".into(),
                    technical_details: format!("Detected in {source_name}: {line}"),
                    relevant_log_lines: snippet,
                    primary_action: None,
                    secondary_actions: Vec::new(),
                    sanitized_log: String::new(),
                };
            }
        }

        // 11. Generic Crash Report fallback
        if let Some((i, &line)) = lines
            .iter()
            .enumerate()
            .find(|(_, l)| l.contains("---- Minecraft Crash Report ----") || l.contains("Description:"))
        {
            return CrashAnalysis {
                is_crash: true,
                category: CrashCategory::GenericCrashReport,
                title: "Server Crash Report Generated".into(),
                plain_english_summary: "The Minecraft server generated a crash dump. Review the highlighted exception lines below.".into(),
                technical_details: format!("Detected in {source_name}: {line}"),
                relevant_log_lines: extract_snippet(&lines, i, 15),
                primary_action: None,
                secondary_actions: Vec::new(),
                sanitized_log: String::new(),
            };
        }

        CrashAnalysis::healthy("No known fatal crash signatures were found in the server logs.")
    }

    /// Queries Modrinth and CurseForge to populate concrete 1-click remediation actions.
    async fn enrich_with_api_resolution(
        analysis: &mut CrashAnalysis,
        instance_dir: &Path,
        mc_version: &str,
        loader_type: &str,
        mod_service: &ModManagementService,
    ) {
        match analysis.category {
            CrashCategory::MissingDependency => {
                let identifier = extract_mod_slug_from_analysis(analysis);
                if let Some(raw_slug) = identifier {
                    let slug = normalize_mod_slug(&raw_slug);

                    // 1. Try Modrinth project versions directly with normalized slug
                    if let Ok(versions) = mod_service
                        .modrinth()
                        .list_project_versions(&slug, Some(mc_version), Some(loader_type))
                        .await
                    {
                        if let Some(ver) = versions.first() {
                            if let Some(primary_file) = ver.files.iter().find(|f| f.primary).or_else(|| ver.files.first()) {
                                let display_name = if ver.name.is_empty() { slug.clone() } else { ver.name.clone() };
                                analysis.primary_action = Some(CrashRemediationAction::InstallMissingMod {
                                    provider: "modrinth".into(),
                                    project_id: slug.clone(),
                                    project_name: display_name.clone(),
                                    file_name: primary_file.filename.clone(),
                                    download_url: primary_file.url.clone(),
                                    version_number: ver.version_number.clone(),
                                });
                                analysis.title = format!("Missing Dependency: {} (1-Click Install Available)", display_name);
                                return;
                            }
                        }
                    }

                    // 2. Try Modrinth search by query
                    if let Ok(hits) = mod_service
                        .modrinth()
                        .search_mods(&slug, Some(mc_version), Some(loader_type))
                        .await
                    {
                        if let Some(first_hit) = hits.first() {
                            if let Ok(versions) = mod_service
                                .modrinth()
                                .list_project_versions(&first_hit.project_id, Some(mc_version), Some(loader_type))
                                .await
                            {
                                if let Some(ver) = versions.first() {
                                    if let Some(primary_file) = ver.files.iter().find(|f| f.primary).or_else(|| ver.files.first()) {
                                        analysis.primary_action = Some(CrashRemediationAction::InstallMissingMod {
                                            provider: "modrinth".into(),
                                            project_id: first_hit.project_id.clone(),
                                            project_name: first_hit.title.clone(),
                                            file_name: primary_file.filename.clone(),
                                            download_url: primary_file.url.clone(),
                                            version_number: ver.version_number.clone(),
                                        });
                                        analysis.title = format!("Missing Dependency: {} (1-Click Install Available)", first_hit.title);
                                        return;
                                    }
                                }
                            }
                        }
                    }

                    // 3. Fallback to CurseForge search
                    if let Ok(hits) = mod_service
                        .curse_forge()
                        .search_mods_with_type(&slug, Some(mc_version), Some(loader_type), Some("mod"))
                        .await
                    {
                        if let Some(first_hit) = hits.first() {
                            // Find latest matching file
                            if let Some(file) = first_hit.latest_files.iter().find(|f| {
                                f.game_versions.iter().any(|v| v == mc_version)
                            }).or_else(|| first_hit.latest_files.first()) {
                                if !file.download_url.is_empty() {
                                    analysis.primary_action = Some(CrashRemediationAction::InstallMissingMod {
                                        provider: "curseforge".into(),
                                        project_id: first_hit.id.to_string(),
                                        project_name: first_hit.name.clone(),
                                        file_name: file.file_name.clone(),
                                        download_url: file.download_url.clone(),
                                        version_number: file.display_name.clone(),
                                    });
                                    analysis.title = format!("Missing Dependency: {} (1-Click Install Available)", first_hit.name);
                                }
                            }
                        }
                    }
                }
            }
            CrashCategory::DuplicateMods => {
                // Inspect instance `mods/` directory for duplicate filenames
                let mods_dir = instance_dir.join("mods");
                if let Ok(entries) = fs::read_dir(&mods_dir) {
                    let files: Vec<PathBuf> = entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.extension().map(|e| e == "jar").unwrap_or(false))
                        .collect();

                    // Check for common prefix patterns (e.g. jei-1.20.4-1.jar and jei-1.20.4-2.jar)
                    for (i, a) in files.iter().enumerate() {
                        for b in files.iter().skip(i + 1) {
                            let name_a = a.file_name().unwrap_or_default().to_string_lossy();
                            let name_b = b.file_name().unwrap_or_default().to_string_lossy();
                            let prefix_a = name_a.split('-').next().unwrap_or("");
                            let prefix_b = name_b.split('-').next().unwrap_or("");
                            if !prefix_a.is_empty() && prefix_a == prefix_b && prefix_a != "minecraft" {
                                analysis.primary_action = Some(CrashRemediationAction::RemoveDuplicateMod {
                                    file_name: name_a.to_string(),
                                    keep_file_name: name_b.to_string(),
                                });
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Executes a pre-computed 1-click remediation action on the instance directory.
    pub async fn execute_remediation(
        instance_dir: &Path,
        action: &CrashRemediationAction,
        mod_service: &ModManagementService,
        bom_service: &BomService,
    ) -> Result<String, String> {
        match action {
            CrashRemediationAction::InstallMissingMod {
                provider,
                file_name,
                download_url,
                version_number,
                project_name,
                ..
            } => {
                info!("1-Click Fix: Installing missing mod '{project_name}' ({file_name}) from {provider}");
                let client = reqwest::Client::new();
                let res = client
                    .get(download_url)
                    .send()
                    .await
                    .map_err(|e| format!("Download request failed: {e}"))?;

                if !res.status().is_success() {
                    return Err(format!("Download failed with HTTP status {}", res.status()));
                }

                let bytes = res.bytes().await.map_err(|e| format!("Failed to read mod bytes: {e}"))?;
                mod_service
                    .add_mod(bytes.as_ref(), file_name, Some(provider))
                    .await
                    .map_err(|e| format!("Failed to install mod into BOM: {e}"))?;

                Ok(format!("Installed '{project_name}' ({version_number}) successfully."))
            }
            CrashRemediationAction::UpdateMod {
                old_file_name,
                new_file_name,
                download_url,
                version_number,
                provider,
                ..
            } => {
                info!("1-Click Fix: Updating mod '{old_file_name}' to '{new_file_name}'");
                let client = reqwest::Client::new();
                let res = client
                    .get(download_url)
                    .send()
                    .await
                    .map_err(|e| format!("Download request failed: {e}"))?;

                let bytes = res.bytes().await.map_err(|e| format!("Failed to read mod bytes: {e}"))?;
                
                // Add new mod to disk and BOM
                mod_service
                    .add_mod(bytes.as_ref(), new_file_name, Some(provider))
                    .await
                    .map_err(|e| format!("Failed to install updated mod: {e}"))?;

                // Remove old mod if different name
                if old_file_name != new_file_name {
                    let old_path = instance_dir.join("mods").join(old_file_name);
                    if old_path.exists() {
                        let _ = fs::remove_file(&old_path);
                    }
                    bom_service.with_bom(|bom| {
                        bom.mods.retain(|m| m.filename != *old_file_name);
                    });
                    let _ = bom_service.save();
                }

                Ok(format!("Updated mod to version {version_number}."))
            }
            CrashRemediationAction::DisableMod { file_name, mod_name } => {
                info!("1-Click Fix: Disabling conflicting mod '{mod_name}' ({file_name})");
                let mods_dir = instance_dir.join("mods");
                let original = mods_dir.join(file_name);
                let disabled = mods_dir.join(format!("{file_name}.disabled"));

                if original.exists() {
                    fs::rename(&original, &disabled)
                        .map_err(|e| format!("Failed to disable mod file: {e}"))?;
                }
                bom_service.with_bom(|bom| {
                    bom.mods.retain(|m| m.filename != *file_name);
                });
                let _ = bom_service.save();
                Ok(format!("Disabled mod '{mod_name}'."))
            }
            CrashRemediationAction::RemoveDuplicateMod { file_name, .. } => {
                info!("1-Click Fix: Removing duplicate mod '{file_name}'");
                let target = instance_dir.join("mods").join(file_name);
                if target.exists() {
                    fs::remove_file(&target).map_err(|e| format!("Failed to remove duplicate jar: {e}"))?;
                }
                bom_service.with_bom(|bom| {
                    bom.mods.retain(|m| m.filename != *file_name);
                });
                let _ = bom_service.save();
                Ok(format!("Removed duplicate mod '{file_name}'."))
            }
            CrashRemediationAction::ResetConfigFile { file_path, backup_path } => {
                info!("1-Click Fix: Resetting configuration file '{file_path}' (backup: '{backup_path}')");
                let target = instance_dir.join(file_path);
                let backup = instance_dir.join(backup_path);

                if target.exists() {
                    let _ = fs::copy(&target, &backup);
                    fs::remove_file(&target).map_err(|e| format!("Failed to delete corrupted config: {e}"))?;
                }
                Ok(format!("Reset '{file_path}' to default (backup saved to '{backup_path}')."))
            }
            CrashRemediationAction::AdjustMemory { recommended_gb, .. } => {
                Ok(format!("Memory recommendation set to {recommended_gb} GB."))
            }
            CrashRemediationAction::ChangeJavaVersion { required_major } => {
                Ok(format!("Java version requirement set to Java {required_major}."))
            }
            CrashRemediationAction::AcceptEula => {
                let server_dir = instance_dir.join("server");
                let eula_file = if server_dir.is_dir() {
                    server_dir.join("eula.txt")
                } else {
                    instance_dir.join("eula.txt")
                };
                if let Some(parent) = eula_file.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                fs::write(&eula_file, "# By changing the settings below to TRUE you are indicating your agreement to our EULA.\neula=true\n")
                    .map_err(|e| format!("Failed to write eula.txt: {e}"))?;
                Ok("Minecraft EULA accepted successfully.".to_string())
            }
            CrashRemediationAction::OpenHelpUrl { url, .. } => {
                Ok(format!("Guidance link: {url}"))
            }
        }
    }
}

/// Helper to extract 4–6 lines around a matched index.
fn extract_snippet(lines: &[&str], center: usize, count: usize) -> Vec<String> {
    let start = center.saturating_sub(1);
    let end = (center + count).min(lines.len());
    lines[start..end].iter().map(|s| s.to_string()).collect()
}

/// Extracts missing mod ID / slug from exception lines or stack trace snippets.
pub fn extract_missing_mod_identifier(line: &str, snippet: &[String]) -> Option<String> {
    // Regex matches common dependency exception patterns:
    // "Mod 'xyz' requires mod 'fabric-api'"
    // "requires 'cloth_config'"
    if let Ok(re) = Regex::new(r"requires (?:version [^ ]+ of )?mod '(?P<dep>[a-zA-Z0-9_\-]+)'") {
        if let Some(caps) = re.captures(line) {
            return caps.name("dep").map(|m| m.as_str().to_string());
        }
    }
    if let Ok(re) = Regex::new(r"requires '(?P<dep>[a-zA-Z0-9_\-]+)'") {
        if let Some(caps) = re.captures(line) {
            return caps.name("dep").map(|m| m.as_str().to_string());
        }
    }
    if let Ok(re) = Regex::new(r"Mod ID: '(?P<dep>[a-zA-Z0-9_\-]+)'") {
        if let Some(caps) = re.captures(line) {
            return caps.name("dep").map(|m| m.as_str().to_string());
        }
    }
    if let Ok(re) = Regex::new(r"requires (?:any version of )?(?P<dep>[a-zA-Z0-9_\-]+), which is missing") {
        if let Some(caps) = re.captures(line) {
            return caps.name("dep").map(|m| m.as_str().to_string());
        }
    }

    // Inspect snippet lines
    for snip in snippet {
        // Modern Fabric 0.15+: "- Install fabric-api, any version" or "- Install mod 'xyz'"
        if let Ok(re) = Regex::new(r#"-\s*Install (?:mod )?['"]?(?P<dep>[a-zA-Z0-9_\-]+)['"]?(?:,|\s|\(|$|\.)"#) {
            if let Some(caps) = re.captures(snip) {
                let dep = caps.name("dep").map(|m| m.as_str().to_string());
                if let Some(d) = dep {
                    if d != "mod" && d != "any" && d != "version" {
                        return Some(d);
                    }
                }
            }
        }
        if let Ok(re) = Regex::new(r"mod '(?P<dep>[a-zA-Z0-9_\-]+)' which is missing") {
            if let Some(caps) = re.captures(snip) {
                return caps.name("dep").map(|m| m.as_str().to_string());
            }
        }
        if let Ok(re) = Regex::new(r"Mod ID: '(?P<dep>[a-zA-Z0-9_\-]+)'") {
            if let Some(caps) = re.captures(snip) {
                return caps.name("dep").map(|m| m.as_str().to_string());
            }
        }
        if let Ok(re) = Regex::new(r"requires '(?P<dep>[a-zA-Z0-9_\-]+)'") {
            if let Some(caps) = re.captures(snip) {
                return caps.name("dep").map(|m| m.as_str().to_string());
            }
        }
    }

    None
}

/// Normalizes mod identifier for API lookups (e.g. cloth_config -> cloth-config, balm_forge -> balm).
pub fn normalize_mod_slug(raw: &str) -> String {
    let mut s = raw.trim().to_lowercase().replace('_', "-");
    for suffix in &["-forge", "-fabric", "-neoforge", "-quilt", "-common"] {
        if s.ends_with(suffix) && s.len() > suffix.len() + 2 {
            s = s[..s.len() - suffix.len()].to_string();
            break;
        }
    }
    match s.as_str() {
        "cloth-config2" => "cloth-config".to_string(),
        "fabric" => "fabric-api".to_string(),
        _ => s,
    }
}

/// Helper to get mod slug from diagnosis
fn extract_mod_slug_from_analysis(analysis: &CrashAnalysis) -> Option<String> {
    for line in &analysis.relevant_log_lines {
        if let Some(slug) = extract_missing_mod_identifier(line, &analysis.relevant_log_lines) {
            return Some(slug);
        }
    }
    None
}

/// Maps class version number (e.g. 65.0 -> Java 21).
fn extract_required_java_major(line: &str) -> i32 {
    if let Ok(re) = Regex::new(r"class file version (?P<ver>\d+)") {
        if let Some(caps) = re.captures(line) {
            if let Some(ver_str) = caps.name("ver") {
                if let Ok(num) = ver_str.as_str().parse::<i32>() {
                    return match num {
                        52 => 8,
                        60 => 16,
                        61 => 17,
                        65 => 21,
                        66 => 22,
                        67 => 23,
                        _ if num > 65 => 21 + (num - 65),
                        _ => 17,
                    };
                }
            }
        }
    }
    17
}

/// Extracts config file path from syntax error lines.
fn extract_config_file_path(line: &str) -> Option<String> {
    if let Ok(re) = Regex::new(r"(?:in|file) (?P<path>config[/\\][a-zA-Z0-9_\-\./\\]+\.(?:json|toml))") {
        if let Some(caps) = re.captures(line) {
            return caps.name("path").map(|m| m.as_str().replace('\\', "/"));
        }
    }
    None
}

/// Privacy-preserving log sanitizer: scrubs local username paths, IP addresses,
/// RCON passwords, and auth tokens before returning logs to clients.
pub fn sanitize_log_text(raw: &str) -> String {
    // 1. Scrub Windows username paths: C:\Users\<name>\... -> C:\Users\<user>\...
    let re_win = Regex::new(r"(?i)[a-z]:\\users\\[^\s\\/]+").unwrap();
    let s1 = re_win.replace_all(raw, "C:\\Users\\<user>");

    // 2. Scrub Linux/macOS user paths: /home/<name>/... or /Users/<name>/...
    let re_nix = Regex::new(r"/(?:home|Users)/[a-zA-Z0-9_\-]+").unwrap();
    let s2 = re_nix.replace_all(&s1, "/home/<user>");

    // 3. Scrub IPv4 addresses (excluding 127.0.0.1 and 0.0.0.0)
    let re_ip = Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap();
    let s3 = re_ip.replace_all(&s2, |caps: &regex::Captures| {
        let ip = caps.get(0).map(|m| m.as_str()).unwrap_or("");
        if ip == "127.0.0.1" || ip == "0.0.0.0" {
            ip.to_string()
        } else {
            "<ip-redacted>".to_string()
        }
    });

    // 4. Scrub RCON passwords and secret keys
    let re_rcon = Regex::new(r"(?i)(rcon\.password|password|secret)=([^\s\r\n]+)").unwrap();
    let s4 = re_rcon.replace_all(&s3, "$1=<redacted>");

    // 5. Scrub Auth tokens: Bearer <token> or AUTH <token>
    let re_auth = Regex::new(r"(?i)(Bearer|AUTH)\s+[a-zA-Z0-9_\-\.]{15,}").unwrap();
    let s5 = re_auth.replace_all(&s4, "$1 <token-redacted>");

    s5.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnose_fabric_missing_dependency() {
        let log = r"
[main/INFO]: Loading Minecraft 1.20.4 with Fabric Loader 0.15.11
[main/ERROR]: ModResolutionException: Mod 'create' (create) 0.5.1 requires mod 'fabric-api' (>=0.90.0), which is missing!
        at net.fabricmc.loader.impl.FabricLoaderImpl.load(FabricLoaderImpl.java:200)
";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "logs/latest.log");
        assert!(analysis.is_crash);
        assert_eq!(analysis.category, CrashCategory::MissingDependency);
        assert!(analysis.title.contains("fabric-api"));
        assert!(!analysis.relevant_log_lines.is_empty());
    }

    #[test]
    fn test_diagnose_forge_missing_dependency() {
        let log = r"
[main/ERROR] [neoforge/]: net.neoforged.fml.ModLoadingIssue: Mod 'create' requires 'cloth_config' 13.0.0 or above
";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "logs/latest.log");
        assert!(analysis.is_crash);
        assert_eq!(analysis.category, CrashCategory::MissingDependency);
        assert!(analysis.title.contains("cloth_config"));
    }

    #[test]
    fn test_diagnose_oom() {
        let log = "java.lang.OutOfMemoryError: Java heap space\n    at java.base/java.util.Arrays.copyOf";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "crash-reports/crash-1.txt");
        assert!(analysis.is_crash);
        assert_eq!(analysis.category, CrashCategory::OutOfMemory);
        assert!(analysis.primary_action.is_some());
    }

    #[test]
    fn test_diagnose_java_version_mismatch() {
        let log = "java.lang.UnsupportedClassVersionError: com/example/Mod has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "logs/latest.log");
        assert!(analysis.is_crash);
        assert_eq!(analysis.category, CrashCategory::JavaVersionMismatch);
        assert_eq!(
            analysis.primary_action,
            Some(CrashRemediationAction::ChangeJavaVersion { required_major: 21 })
        );
    }

    #[test]
    fn test_diagnose_corrupted_json() {
        let log = "com.google.gson.JsonSyntaxException: Malformed JSON in config/cloth-config.json: line 5 column 2";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "logs/latest.log");
        assert!(analysis.is_crash);
        assert_eq!(analysis.category, CrashCategory::CorruptedConfig);
        assert_eq!(
            analysis.primary_action,
            Some(CrashRemediationAction::ResetConfigFile {
                file_path: "config/cloth-config.json".into(),
                backup_path: "config/cloth-config.json.bak".into(),
            })
        );
    }

    #[test]
    fn test_benign_errors_do_not_trigger_crash() {
        let log = r"
[main/INFO]: Starting minecraft server version 1.20.4
[Server thread/ERROR] [minecraft/RecipeManager]: Parsing error loading recipe create:crushed_zinc_ore
[Server thread/ERROR] [minecraft/AdvancementList]: Parsing error loading built-in advancement
[Server thread/INFO] [minecraft/DedicatedServer]: Done (14.212s)! For help, type 'help'
[Server thread/INFO] [minecraft/MinecraftServer]: Stopping server
";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "logs/latest.log");
        assert!(!analysis.is_crash, "Benign errors during runtime should NOT trigger a crash!");
    }

    #[test]
    fn test_modern_fabric_solution_format() {
        let log = r"
net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!
A potential solution has been determined:
	 - Install fabric-api, any version.
	 - Install cloth-config2, version 13.0.0 or later.
";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "logs/latest.log");
        assert!(analysis.is_crash);
        assert_eq!(analysis.category, CrashCategory::MissingDependency);
        assert!(analysis.title.contains("fabric-api"));
    }

    #[test]
    fn test_client_only_mod_crash() {
        let log = r"
java.lang.NoClassDefFoundError: net/minecraft/client/gui/screens/Screen
    at com.example.minimap.MinimapMod.onInitializeServer(MinimapMod.java:42)
";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "logs/latest.log");
        assert!(analysis.is_crash);
        assert_eq!(analysis.category, CrashCategory::ClientOnlyModOnServer);
    }

    #[test]
    fn test_eula_crash() {
        let log = "You need to agree to the EULA in order to run the server. Go to eula.txt for more info.";
        let analysis = CrashAnalyzerService::diagnose_heuristics(log, "logs/latest.log");
        assert!(analysis.is_crash);
        assert_eq!(analysis.category, CrashCategory::EulaNotAccepted);
        assert_eq!(analysis.primary_action, Some(CrashRemediationAction::AcceptEula));
    }

    #[test]
    fn test_normalize_mod_slug() {
        assert_eq!(normalize_mod_slug("cloth_config"), "cloth-config");
        assert_eq!(normalize_mod_slug("balm_forge"), "balm");
        assert_eq!(normalize_mod_slug("cloth-config2"), "cloth-config");
        assert_eq!(normalize_mod_slug("fabric"), "fabric-api");
        assert_eq!(normalize_mod_slug("architectury_api"), "architectury-api");
    }
}
