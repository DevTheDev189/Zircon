//! Scans an instance's crash reports and `logs/latest.log` for the most common
//! modded-Minecraft failure signatures and returns a clean, actionable summary
//! for the launcher UI (Settings → Crash Diagnostics).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A structured summary of a detected crash (or the "all clear").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashAnalysis {
    pub is_crash: bool,
    pub category: String,
    pub title: String,
    pub explanation: String,
    pub suggested_fix: String,
    pub relevant_lines: Vec<String>,
}

/// Classifies a log/crash-report payload into a known failure category.
pub fn analyze_log_content(log_text: &str) -> CrashAnalysis {
    let lines: Vec<&str> = log_text.lines().collect();

    // 1. Missing Dependencies
    for (i, &line) in lines.iter().enumerate() {
        if line.contains("requires")
            && (line.contains("of mod")
                || line.contains("Fabric API")
                || line.contains("quilted_fabric_api"))
        {
            return CrashAnalysis {
                is_crash: true,
                category: "MissingDependency".into(),
                title: "Missing Mod Dependency".into(),
                explanation: format!("A mod is missing a required library or API: {line}"),
                suggested_fix: "Install the required mod/API matching your Minecraft version."
                    .into(),
                relevant_lines: extract_snippet(&lines, i, 5),
            };
        }
    }

    // 2. Incompatible Mods
    for (i, &line) in lines.iter().enumerate() {
        if line.contains("IncompatibleModException")
            || line.contains("Incompatible mods found!")
            || line.contains("is incompatible with")
        {
            return CrashAnalysis {
                is_crash: true,
                category: "ModIncompatibility".into(),
                title: "Mod Incompatibility Detected".into(),
                explanation: "Conflicting or incompatible mods are present in the mods folder."
                    .into(),
                suggested_fix:
                    "Check the incompatible mod versions and update or remove conflicting mods."
                        .into(),
                relevant_lines: extract_snippet(&lines, i, 8),
            };
        }
    }

    // 3. Mixin Errors
    for (i, &line) in lines.iter().enumerate() {
        if line.contains("MixinTransformerError") || line.contains("Critical injection failure") {
            return CrashAnalysis {
                is_crash: true,
                category: "MixinError".into(),
                title: "Bytecode Patch Failure (Mixin Conflict)".into(),
                explanation: "A mod failed to inject bytecode into the game.".into(),
                suggested_fix:
                    "Identify the mod in the Mixin error and check for updates or conflicting mods."
                        .into(),
                relevant_lines: extract_snippet(&lines, i, 6),
            };
        }
    }

    // 4. Java Version Mismatch
    for (_, &line) in lines.iter().enumerate() {
        if line.contains("UnsupportedClassVersionError")
            || line.contains("has been compiled by a more recent version of the Java Runtime")
        {
            return CrashAnalysis {
                is_crash: true,
                category: "JavaVersionMismatch".into(),
                title: "Incompatible Java Version".into(),
                explanation: "A mod was compiled for a newer Java version than currently running."
                    .into(),
                suggested_fix: "Select Java 17 (MC 1.18–1.20.4) or Java 21+ (MC 1.20.5+).".into(),
                relevant_lines: vec![line.to_string()],
            };
        }
    }

    // 5. Out Of Memory
    for (_, &line) in lines.iter().enumerate() {
        if line.contains("java.lang.OutOfMemoryError")
            || line.contains("GC overhead limit exceeded")
        {
            return CrashAnalysis {
                is_crash: true,
                category: "OutOfMemory".into(),
                title: "Out of Memory (OOM)".into(),
                explanation: "Minecraft exhausted its allocated RAM heap.".into(),
                suggested_fix: "Increase the memory slider in Settings.".into(),
                relevant_lines: vec![line.to_string()],
            };
        }
    }

    // 6. Generic Crash Report
    if let Some((i, &line)) = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("---- Minecraft Crash Report ----") || l.contains("Description:"))
    {
        return CrashAnalysis {
            is_crash: true,
            category: "CrashReport".into(),
            title: "Game Crash Report".into(),
            explanation: line.to_string(),
            suggested_fix: "Review the crash details below to locate the offending mod.".into(),
            relevant_lines: extract_snippet(&lines, i, 15),
        };
    }

    CrashAnalysis {
        is_crash: false,
        category: "None".into(),
        title: "No Crashes Detected".into(),
        explanation: "No known fatal error patterns were detected in the log.".into(),
        suggested_fix: String::new(),
        relevant_lines: Vec::new(),
    }
}

/// A few lines around `center` (starting one line before it) for context.
fn extract_snippet(lines: &[&str], center: usize, count: usize) -> Vec<String> {
    let start = center.saturating_sub(1);
    let end = (center + count).min(lines.len());
    lines[start..end].iter().map(|s| s.to_string()).collect()
}

/// Analyzes the newest crash report in `crash-reports/`, falling back to
/// `logs/latest.log` when no crash report matches.
pub fn analyze_instance_latest_crash(game_dir: &Path) -> Option<CrashAnalysis> {
    let crash_dir = game_dir.join("crash-reports");
    if crash_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&crash_dir) {
            let mut files: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            files.sort();
            if let Some(latest) = files.last() {
                if let Ok(content) = std::fs::read_to_string(latest) {
                    let analysis = analyze_log_content(&content);
                    if analysis.is_crash {
                        return Some(analysis);
                    }
                }
            }
        }
    }
    let log_file = game_dir.join("logs").join("latest.log");
    if log_file.is_file() {
        if let Ok(content) = std::fs::read_to_string(&log_file) {
            let analysis = analyze_log_content(&content);
            if analysis.is_crash {
                return Some(analysis);
            }
        }
    }
    None
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
        assert_eq!("MissingDependency", analysis.category);
        assert!(!analysis.relevant_lines.is_empty());
    }

    #[test]
    fn detects_mixin_failure() {
        let log = "Mixin apply failed\nCritical injection failure: @Inject at xyz\nstack";
        let analysis = analyze_log_content(log);
        assert!(analysis.is_crash);
        assert_eq!("MixinError", analysis.category);
    }

    #[test]
    fn detects_java_version_mismatch() {
        let log = "java.lang.UnsupportedClassVersionError: mod has been compiled by a more \
                   recent version of the Java Runtime";
        let analysis = analyze_log_content(log);
        assert!(analysis.is_crash);
        assert_eq!("JavaVersionMismatch", analysis.category);
    }

    #[test]
    fn detects_oom() {
        let log = "Caused by: java.lang.OutOfMemoryError: Java heap space";
        let analysis = analyze_log_content(log);
        assert!(analysis.is_crash);
        assert_eq!("OutOfMemory", analysis.category);
    }

    #[test]
    fn clean_log_reports_no_crash() {
        let analysis = analyze_log_content("preparing spawn area\nDone (1.234s)!");
        assert!(!analysis.is_crash);
        assert_eq!("None", analysis.category);
    }
}
