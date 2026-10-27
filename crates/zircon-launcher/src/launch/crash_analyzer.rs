//! Smart Crash Diagnostic Engine for Minecraft client instances.
//!
//! Scans crash reports and `logs/latest.log` for known failure signatures
//! (missing dependencies, mod conflicts, Mixin failures, Java mismatches, OOM),
//! integrates with Modrinth API to pre-resolve missing parent libraries, and
//! produces actionable 1-click remediation payloads for the launcher UI.

use std::path::{Path, PathBuf};
use regex::Regex;
use serde::{Deserialize, Serialize};
use zircon_core::api::modrinth::ModrinthApiClient;

/// High-level diagnostic classification of a crash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CrashCategory {
    MissingDependency,
    ModIncompatibility,
    MixinError,
    OutOfMemory,
    JavaVersionMismatch,
    CorruptedConfig,
    DuplicateMods,
    CrashReport,
    None,
}

impl Default for CrashCategory {
    fn default() -> Self {
        Self::None
    }
}

/// Action payload for 1-click automated remediation in the launcher UI.
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
    RollbackSnapshot {
        snapshot_filename: String,
        label: String,
    },
    ChangeJavaVersion {
        required_major: i32,
    },
}

/// A structured summary of a detected crash with plain English explanation,
/// sanitized log traces, and 1-click remediation actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashAnalysis {
    pub is_crash: bool,
    pub category: CrashCategory,
    pub title: String,
    pub plain_english_summary: String,
    pub explanation: String,
    pub technical_details: String,
    pub suggested_fix: String,
    pub relevant_lines: Vec<String>,
    pub primary_action: Option<CrashRemediationAction>,
    pub secondary_actions: Vec<CrashRemediationAction>,
    pub sanitized_log: String,
}

impl CrashAnalysis {
    pub fn healthy(summary: &str) -> Self {
        Self {
            is_crash: false,
            category: CrashCategory::None,
            title: "No Crashes Detected".into(),
            plain_english_summary: summary.into(),
            explanation: summary.into(),
            technical_details: String::new(),
            suggested_fix: String::new(),
            relevant_lines: Vec::new(),
            primary_action: None,
            secondary_actions: Vec::new(),
            sanitized_log: String::new(),
        }
    }

    /// Sanitizes all human-facing text fields in case raw Minecraft JSON chat components
    /// were passed directly as error messages.
    pub fn sanitize_text_fields(&mut self) {
        self.title = strip_minecraft_json_text(&self.title);
        self.plain_english_summary = strip_minecraft_json_text(&self.plain_english_summary);
        self.explanation = strip_minecraft_json_text(&self.explanation);
        self.suggested_fix = strip_minecraft_json_text(&self.suggested_fix);
    }
}

/// Classifies a log or crash report text into a known failure category and produces
/// preliminary remediation actions.
pub fn analyze_log_content(log_text: &str) -> CrashAnalysis {
    let mut analysis = analyze_log_content_raw(log_text);
    analysis.sanitize_text_fields();
    analysis
}

fn analyze_log_content_raw(log_text: &str) -> CrashAnalysis {
    let lines: Vec<&str> = log_text.lines().collect();

    // 1. Missing Dependencies (Fabric/Quilt, NeoForge, Forge)
    for (i, &line) in lines.iter().enumerate() {
        if line.contains("ModResolutionException")
            || line.contains("ModLoadingIssue")
            || line.contains("MissingMandatoryDependenciesException")
            || line.contains("Missing or unsupported mandatory dependencies")
            || line.contains("A potential solution has been determined")
            || (line.contains("requires")
                && (line.contains("which is missing")
                    || line.contains("of mod")
                    || line.contains("Fabric API")
                    || line.contains("quilted_fabric_api")
                    || line.contains("or above")
                    || line.contains("any version of")))
        {
            let snippet = extract_snippet(&lines, i, 8);
            let missing_slug = extract_missing_mod_identifier(line, &snippet);
            let mod_display = missing_slug.clone().unwrap_or_else(|| "Required Library".to_string());
            let title = format!("Missing Mod Dependency: {mod_display}");
            let explanation = if let Some(ref slug) = missing_slug {
                format!("The game failed to launch because the required dependency '{slug}' is missing from your mods folder.")
            } else {
                format!("A required library or API is missing: {line}")
            };
            let suggested_fix = format!("Install {mod_display} matching your Minecraft version and loader.");

            return CrashAnalysis {
                is_crash: true,
                category: CrashCategory::MissingDependency,
                title,
                plain_english_summary: explanation.clone(),
                explanation,
                technical_details: line.to_string(),
                suggested_fix,
                relevant_lines: snippet,
                primary_action: None, // Filled during async Modrinth API enrichment
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
            let title = "Duplicate Mod Files Detected".into();
            let explanation = "Multiple versions of the same mod were found in your mods folder.".into();
            let suggested_fix = "Remove the older or duplicate version of the mod.".into();

            return CrashAnalysis {
                is_crash: true,
                category: CrashCategory::DuplicateMods,
                title,
                plain_english_summary: explanation,
                explanation: "Multiple versions of the same mod exist in mods/.".into(),
                technical_details: line.to_string(),
                suggested_fix,
                relevant_lines: snippet,
                primary_action: None,
                secondary_actions: Vec::new(),
                sanitized_log: String::new(),
            };
        }
    }

    // 3. Known Incompatible Mods & Mixin Conflicts (e.g. OptiFine with Sodium/Iris)
    for (i, &line) in lines.iter().enumerate() {
        let has_incompat = line.contains("IncompatibleModException")
            || line.contains("Incompatible mods found!")
            || line.contains("Incompatible mods found")
            || line.contains("is incompatible with")
            || (line.contains("requires version") && line.contains("but only"));

        let has_optifine_conflict = line.to_lowercase().contains("optifine")
            && (line.to_lowercase().contains("sodium") || line.to_lowercase().contains("iris") || line.to_lowercase().contains("rubidium") || line.to_lowercase().contains("oculus"));

        if has_incompat || has_optifine_conflict {
            let snippet = extract_snippet(&lines, i, 8);
            let (title, explanation, suggested_fix): (String, String, String) = if has_optifine_conflict {
                (
                    "OptiFine Incompatibility with Sodium/Iris".into(),
                    "OptiFine is fundamentally incompatible with Sodium, Iris, or modern shader engines.".into(),
                    "Disable or remove OptiFine; Sodium and Iris provide superior performance and shader support.".into()
                )
            } else {
                (
                    "Mod Incompatibility Detected".into(),
                    "Conflicting or incompatible mods are present in your mods folder.".into(),
                    "Check the incompatible mod versions and update or disable conflicting mods.".into()
                )
            };

            return CrashAnalysis {
                is_crash: true,
                category: CrashCategory::ModIncompatibility,
                title,
                plain_english_summary: explanation.clone(),
                explanation,
                technical_details: line.to_string(),
                suggested_fix,
                relevant_lines: snippet,
                primary_action: None,
                secondary_actions: Vec::new(),
                sanitized_log: String::new(),
            };
        }
    }

    // 4. Mixin / Bytecode Injection Failures
    for (i, &line) in lines.iter().enumerate() {
        if line.contains("MixinTransformerError")
            || line.contains("Critical injection failure")
            || line.contains("MixinApplyError")
            || line.contains("org.spongepowered.asm.mixin.transformer")
        {
            let snippet = extract_snippet(&lines, i, 8);
            let title: String = "Bytecode Patch Failure (Mixin Conflict)".into();
            let explanation: String = "A mod failed to inject bytecode into Minecraft, usually due to a conflicting mod or wrong game version.".into();
            let suggested_fix: String = "Identify the mod referenced in the Mixin trace and check for updates or disable it.".into();

            return CrashAnalysis {
                is_crash: true,
                category: CrashCategory::MixinError,
                title,
                plain_english_summary: explanation.clone(),
                explanation,
                technical_details: line.to_string(),
                suggested_fix,
                relevant_lines: snippet,
                primary_action: None,
                secondary_actions: Vec::new(),
                sanitized_log: String::new(),
            };
        }
    }

    // 5. Corrupted Configuration / JSON / TOML files
    for (i, &line) in lines.iter().enumerate() {
        if line.contains("JsonSyntaxException")
            || line.contains("TomlDecodingException")
            || line.contains("MalformedJsonException")
            || line.contains("Failed to parse config")
        {
            let file_path = extract_config_file_path(line);
            let snippet = extract_snippet(&lines, i, 6);
            let title = "Corrupted Mod Configuration File".into();
            let explanation = format!(
                "A configuration file ({}) contains invalid syntax and crashed the game.",
                file_path.as_deref().unwrap_or("config file")
            );
            let suggested_fix = "Reset the corrupted configuration file to default settings.".into();

            let primary = file_path.map(|fp| CrashRemediationAction::ResetConfigFile {
                file_path: fp.clone(),
                backup_path: format!("{fp}.bak"),
            });

            return CrashAnalysis {
                is_crash: true,
                category: CrashCategory::CorruptedConfig,
                title,
                plain_english_summary: explanation.clone(),
                explanation,
                technical_details: line.to_string(),
                suggested_fix,
                relevant_lines: snippet,
                primary_action: primary,
                secondary_actions: Vec::new(),
                sanitized_log: String::new(),
            };
        }
    }

    // 6. Java Version Mismatch
    for (i, &line) in lines.iter().enumerate() {
        if line.contains("UnsupportedClassVersionError")
            || line.contains("has been compiled by a more recent version of the Java Runtime")
        {
            let req_major = extract_required_java_major(line);
            let title = format!("Java Runtime Mismatch (Requires Java {req_major})");
            let explanation = format!(
                "A mod was compiled for Java {req_major}, but Minecraft is running with an older Java runtime."
            );
            let suggested_fix = format!("Switch your instance Java settings to Java {req_major}+.");

            return CrashAnalysis {
                is_crash: true,
                category: CrashCategory::JavaVersionMismatch,
                title,
                plain_english_summary: explanation.clone(),
                explanation,
                technical_details: line.to_string(),
                suggested_fix,
                relevant_lines: extract_snippet(&lines, i, 4),
                primary_action: Some(CrashRemediationAction::ChangeJavaVersion {
                    required_major: req_major,
                }),
                secondary_actions: Vec::new(),
                sanitized_log: String::new(),
            };
        }
    }

    // 7. Out Of Memory (OOM) Heap Exhaustion
    for (i, &line) in lines.iter().enumerate() {
        if line.contains("java.lang.OutOfMemoryError")
            || line.contains("GC overhead limit exceeded")
            || line.contains("Java heap space")
        {
            let title: String = "Out of Memory (Heap Exhausted)".into();
            let explanation: String = "Minecraft ran out of allocated RAM heap memory during launch or gameplay.".into();
            let suggested_fix: String = "Increase allocated memory in Instance Settings to 4GB or higher.".into();

            return CrashAnalysis {
                is_crash: true,
                category: CrashCategory::OutOfMemory,
                title,
                plain_english_summary: explanation.clone(),
                explanation,
                technical_details: line.to_string(),
                suggested_fix,
                relevant_lines: extract_snippet(&lines, i, 4),
                primary_action: Some(CrashRemediationAction::AdjustMemory {
                    current_gb: 2,
                    recommended_gb: 4,
                }),
                secondary_actions: Vec::new(),
                sanitized_log: String::new(),
            };
        }
    }

    // 8. Generic Minecraft Crash Report Fallback
    if let Some((i, &line)) = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("---- Minecraft Crash Report ----") || l.contains("Description:"))
    {
        return CrashAnalysis {
            is_crash: true,
            category: CrashCategory::CrashReport,
            title: "Game Crash Report Generated".into(),
            plain_english_summary: format!("Minecraft crashed with error: {line}"),
            explanation: line.to_string(),
            technical_details: line.to_string(),
            suggested_fix: "Review the crash details below to locate the offending mod.".into(),
            relevant_lines: extract_snippet(&lines, i, 15),
            primary_action: None,
            secondary_actions: Vec::new(),
            sanitized_log: String::new(),
        };
    }

    CrashAnalysis::healthy("No known fatal crash signatures were detected in the log.")
}

/// Extracts a few lines around `center` for error context.
fn extract_snippet(lines: &[&str], center: usize, count: usize) -> Vec<String> {
    let start = center.saturating_sub(1);
    let end = (center + count).min(lines.len());
    lines[start..end].iter().map(|s| s.to_string()).collect()
}

/// Analyzes the newest crash report in `crash-reports/`, falling back to `logs/latest.log`.
pub fn analyze_instance_latest_crash(game_dir: &Path) -> Option<CrashAnalysis> {
    let crash_dir = game_dir.join("crash-reports");
    if crash_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&crash_dir) {
            let mut files: Vec<(PathBuf, std::time::SystemTime)> = entries
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.is_file() {
                        let mod_time = e.metadata().ok().and_then(|m| m.modified().ok())?;
                        Some((p, mod_time))
                    } else {
                        None
                    }
                })
                .collect();

            // Sort newest modified first
            files.sort_by(|a, b| b.1.cmp(&a.1));

            if let Some((latest, _)) = files.first() {
                if let Ok(content) = std::fs::read_to_string(latest) {
                    let mut analysis = analyze_log_content(&content);
                    if analysis.is_crash {
                        analysis.sanitized_log = sanitize_log_text(&content);
                        return Some(analysis);
                    }
                }
            }
        }
    }

    let log_file = game_dir.join("logs").join("latest.log");
    if log_file.is_file() {
        if let Ok(content) = std::fs::read_to_string(&log_file) {
            let mut analysis = analyze_log_content(&content);
            if analysis.is_crash {
                analysis.sanitized_log = sanitize_log_text(&content);
                return Some(analysis);
            }
        }
    }
    None
}

/// Asynchronously enriches a crash analysis by:
/// 1. Querying Modrinth for missing dependency download URLs.
/// 2. Discovering offending mod files on disk to propose disabling them.
/// 3. Attaching the latest working Time Machine snapshot for 1-click rollback.
pub async fn enrich_crash_analysis(
    analysis: &mut CrashAnalysis,
    game_dir: &Path,
    mc_version: Option<&str>,
    loader_type: Option<&str>,
    modrinth: Option<&ModrinthApiClient>,
) {
    // 1. Missing Dependency Modrinth API Resolution
    if analysis.category == CrashCategory::MissingDependency && analysis.primary_action.is_none() {
        if let Some(slug) = extract_mod_slug_from_analysis(analysis) {
            let normalized = normalize_mod_slug(&slug);
            if let Some(client) = modrinth {
                // Try searching Modrinth for the project
                if let Ok(hits) = client.search_mods(&normalized, mc_version, loader_type).await {
                    if let Some(first_hit) = hits.first() {
                        if let Ok(versions) = client
                            .list_project_versions(&first_hit.project_id, mc_version, loader_type)
                            .await
                        {
                            if let Some(ver) = versions.first() {
                                if let Some(primary_file) = ver.files.iter().find(|f| f.primary).or_else(|| ver.files.first()) {
                                    let display_title = if !ver.name.is_empty() {
                                        ver.name.clone()
                                    } else {
                                        first_hit.title.clone()
                                    };
                                    analysis.primary_action = Some(CrashRemediationAction::InstallMissingMod {
                                        provider: "modrinth".into(),
                                        project_id: first_hit.project_id.clone(),
                                        project_name: display_title.clone(),
                                        file_name: primary_file.filename.clone(),
                                        download_url: primary_file.url.clone(),
                                        version_number: ver.version_number.clone(),
                                    });
                                    analysis.title = format!("Missing Dependency: {} (1-Click Install Available)", display_title);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Mod Incompatibility & Mixin Offending File Detection
    if (analysis.category == CrashCategory::ModIncompatibility || analysis.category == CrashCategory::MixinError)
        && analysis.primary_action.is_none()
    {
        // Check for OptiFine
        let mods_dir = game_dir.join("mods");
        if mods_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&mods_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().contains("optifine") && name.ends_with(".jar") {
                        analysis.primary_action = Some(CrashRemediationAction::DisableMod {
                            file_name: name.clone(),
                            mod_name: "OptiFine".to_string(),
                        });
                        break;
                    }
                }
            }
        }
    }

    // 3. Attach Time Machine Snapshot Rollback as secondary action
    if let Ok(snapshots) = crate::snapshots::list_snapshots(game_dir) {
        if let Some(recent) = snapshots.first() {
            analysis.secondary_actions.push(CrashRemediationAction::RollbackSnapshot {
                snapshot_filename: recent.filename.clone(),
                label: recent.label.clone(),
            });
        }
    }
}

/// Extracts missing mod ID / slug from exception lines or stack trace snippets.
pub fn extract_missing_mod_identifier(line: &str, snippet: &[String]) -> Option<String> {
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
    for line in &analysis.relevant_lines {
        if let Some(slug) = extract_missing_mod_identifier(line, &analysis.relevant_lines) {
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

/// If a string contains or is a serialized Minecraft JSON text component (e.g. `{"text": "...", "extra": [...]}`),
/// parses and extracts plain text elements cleanly into human-readable text.
pub fn strip_minecraft_json_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.contains('{') || !trimmed.contains('}') {
        return trimmed.to_string();
    }

    // Direct JSON object check
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let mut out = String::new();
            extract_text_from_json(&value, &mut out);
            let cleaned = out.trim();
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }
    }

    // Substring JSON component check (e.g. "Minecraft crashed: {"text": "..."}")
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            let candidate = &trimmed[start..=end];
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) {
                let mut out = String::new();
                extract_text_from_json(&value, &mut out);
                let cleaned = out.trim();
                if !cleaned.is_empty() {
                    let mut prefix = trimmed[..start].trim().to_string();
                    if !prefix.is_empty() {
                        prefix.push(' ');
                    }
                    prefix.push_str(cleaned);
                    return prefix;
                }
            }
        }
    }

    trimmed.to_string()
}

fn extract_text_from_json(val: &serde_json::Value, out: &mut String) {
    match val {
        serde_json::Value::String(s) => {
            out.push_str(s);
        }
        serde_json::Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                out.push_str(text);
            }
            if let Some(translate) = map.get("translate").and_then(|v| v.as_str()) {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(translate);
            }
            if let Some(extra) = map.get("extra").and_then(|v| v.as_array()) {
                for item in extra {
                    extract_text_from_json(item, out);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_text_from_json(item, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_dependency() {
        let log =
            "some line\n[main/ERROR] Mod 'foobar' requires version 0.14.0 of mod 'fabric_api' \
                   which is missing!\nmore lines\nmore lines\nmore lines\nmore lines";
        let analysis = analyze_log_content(log);
        assert!(analysis.is_crash);
        assert_eq!(CrashCategory::MissingDependency, analysis.category);
        assert!(!analysis.relevant_lines.is_empty());
    }

    #[test]
    fn detects_mixin_failure() {
        let log = "Mixin apply failed\nCritical injection failure: @Inject at xyz\nstack";
        let analysis = analyze_log_content(log);
        assert!(analysis.is_crash);
        assert_eq!(CrashCategory::MixinError, analysis.category);
    }

    #[test]
    fn detects_java_version_mismatch() {
        let log = "java.lang.UnsupportedClassVersionError: mod has been compiled by a more \
                   recent version of the Java Runtime (class file version 65.0)";
        let analysis = analyze_log_content(log);
        assert!(analysis.is_crash);
        assert_eq!(CrashCategory::JavaVersionMismatch, analysis.category);
        assert_eq!(
            analysis.primary_action,
            Some(CrashRemediationAction::ChangeJavaVersion { required_major: 21 })
        );
    }

    #[test]
    fn detects_oom() {
        let log = "Caused by: java.lang.OutOfMemoryError: Java heap space";
        let analysis = analyze_log_content(log);
        assert!(analysis.is_crash);
        assert_eq!(CrashCategory::OutOfMemory, analysis.category);
        assert_eq!(
            analysis.primary_action,
            Some(CrashRemediationAction::AdjustMemory {
                current_gb: 2,
                recommended_gb: 4,
            })
        );
    }

    #[test]
    fn clean_log_reports_no_crash() {
        let analysis = analyze_log_content("preparing spawn area\nDone (1.234s)!");
        assert!(!analysis.is_crash);
        assert_eq!(CrashCategory::None, analysis.category);
    }

    #[test]
    fn strips_minecraft_json_text_components() {
        let raw_json = r#"{"text": "Failed to connect", "extra": [{"text": ": Invalid player session"}]}"#;
        let stripped = strip_minecraft_json_text(raw_json);
        assert_eq!(stripped, "Failed to connect: Invalid player session");

        let embedded = r#"Exception thrown: {"text": "Mod initialization error", "extra": [{"text": " in Sodium"}]}"#;
        let stripped_embedded = strip_minecraft_json_text(embedded);
        assert_eq!(stripped_embedded, "Exception thrown: Mod initialization error in Sodium");

        let plain = "Simple plain text error message";
        assert_eq!(strip_minecraft_json_text(plain), plain);
    }
}
