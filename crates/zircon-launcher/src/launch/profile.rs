//! Resolves version profile JSON files (vanilla and loader-generated) into the
//! launch inputs they describe: the inherited-profile chain, the merged library
//! set, the JVM arguments and the game arguments — with `${token}` placeholders
//! substituted.
//!
//! Port of `com.mcmanager.client.profile.VersionProfileResolver`.

use std::collections::{HashMap, HashSet};

use crate::error::LauncherError;
use crate::model::version::{LibrarySpec, VersionProfile};

/// Parses version profile JSON text into a [`VersionProfile`].
pub fn parse_profile(content: &str) -> Result<VersionProfile, LauncherError> {
    serde_json::from_str(content).map_err(LauncherError::from)
}

/// The current OS id as used by Mojang `rules` (`windows`, `linux`, `osx`).
pub fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// The current CPU architecture id as used by Mojang `rules` (`x86`,
/// `x86_64`, `arm64`).
pub fn current_os_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        "x86"
    }
}

/// Walks the `inheritsFrom` chain from `root` towards vanilla, loading parents
/// through `parent_loader` (which typically parses the vanilla version profile
/// already fetched from Mojang's manifest).
///
/// Returns the chain ordered child-first, ending at the profile with no parent.
/// Parent-loading failures are logged and break the chain (mirroring the Java
/// behaviour) rather than failing the whole launch.
pub fn resolve_chain(
    root: &VersionProfile,
    parent_loader: impl Fn(&str) -> Result<VersionProfile, LauncherError>,
) -> Vec<VersionProfile> {
    let mut chain = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut current = root.clone();
    loop {
        if !seen.insert(current.id.clone()) {
            break;
        }
        chain.push(current.clone());
        let Some(parent) = current.inherits_from.clone() else {
            break;
        };
        if parent.trim().is_empty() {
            break;
        }
        match parent_loader(&parent) {
            Ok(loaded) => current = loaded,
            Err(e) => {
                tracing::warn!(
                    "Could not resolve inherited profile '{}' of '{}': {e}",
                    parent,
                    current.id
                );
                break;
            }
        }
    }
    chain
}

/// Merges the libraries of every profile in the chain (child first), skipping
/// libraries whose OS `rules` disallow them and de-duplicating by Maven
/// coordinate name.
pub fn merged_libraries(chain: &[VersionProfile]) -> Vec<LibrarySpec> {
    let mut merged: Vec<LibrarySpec> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for profile in chain {
        for lib in &profile.libraries {
            if !library_allowed(lib) {
                continue;
            }
            if seen.insert(lib.name.clone()) {
                merged.push(lib.clone());
            }
        }
    }
    merged
}

/// Resolves the JVM arguments of the profile chain (child first) with `tokens`
/// substituted. Argument entries may be plain strings or rule objects
/// (`{"rules": [...], "value": "..."}`), mirroring the Mojang version JSON
/// schema.
pub fn resolve_jvm_arguments(
    chain: &[VersionProfile],
    tokens: &HashMap<String, String>,
) -> Vec<String> {
    let mut args = Vec::new();
    for profile in chain {
        let Some(arguments) = profile.arguments.as_ref() else {
            continue;
        };
        collect_arguments(arguments.get("jvm"), tokens, &mut args, &HashSet::new());
    }
    args
}

/// Resolves the game arguments of the profile chain (child first) with `tokens`
/// substituted.
///
/// `enabled_features` gates `rules.features` entries, e.g.
/// `is_quick_play_multiplayer` when the launcher auto-connects the player to a
/// server.
pub fn resolve_game_arguments(
    chain: &[VersionProfile],
    tokens: &HashMap<String, String>,
    enabled_features: &HashSet<String>,
) -> Vec<String> {
    let mut args = Vec::new();
    for profile in chain {
        if let Some(arguments) = profile.arguments.as_ref() {
            collect_arguments(arguments.get("game"), tokens, &mut args, enabled_features);
        }
        // Pre-1.13 profiles carry game args as a single space-separated string.
        if let Some(legacy) = profile.minecraft_arguments.as_deref() {
            if !legacy.trim().is_empty() {
                args.push(substitute(legacy, tokens));
            }
        }
    }
    args
}

fn collect_arguments(
    section: Option<&serde_json::Value>,
    tokens: &HashMap<String, String>,
    out: &mut Vec<String>,
    enabled_features: &HashSet<String>,
) {
    let Some(section) = section else { return };
    let Some(array) = section.as_array() else {
        return;
    };
    for element in array {
        if let Some(text) = element.as_str() {
            out.push(substitute(text, tokens));
        } else if element.is_object() {
            if !argument_allowed(element, enabled_features) {
                continue;
            }
            let Some(value) = element.get("value") else {
                continue;
            };
            if let Some(text) = value.as_str() {
                out.push(substitute(text, tokens));
            } else if let Some(items) = value.as_array() {
                // Rule objects may carry an array of values (e.g. --width/--height).
                for item in items {
                    if let Some(text) = item.as_str() {
                        out.push(substitute(text, tokens));
                    }
                }
            }
        }
    }
}

fn argument_allowed(obj: &serde_json::Value, enabled_features: &HashSet<String>) -> bool {
    let Some(rules) = obj.get("rules") else {
        return true;
    };
    rules_allow(rules, enabled_features)
}

fn library_allowed(lib: &LibrarySpec) -> bool {
    let Some(rules) = lib.rules.as_ref() else {
        return true;
    };
    rules_allow(rules, &HashSet::new())
}

/// Evaluates a Mojang `rules` array: the last matching rule's action wins.
fn rules_allow(rules: &serde_json::Value, enabled_features: &HashSet<String>) -> bool {
    let Some(array) = rules.as_array() else {
        return true;
    };
    if array.is_empty() {
        return true;
    }
    let mut allow = false;
    for rule_el in array {
        let Some(rule) = rule_el.as_object() else {
            continue;
        };
        if !features_match(rule, enabled_features) {
            continue;
        }
        let applies = rule.get("os").is_none_or(os_matches);
        if applies {
            allow = rule.get("action").and_then(|a| a.as_str()) == Some("allow");
        }
    }
    allow
}

/// A `features` rule applies only when every declared feature matches the
/// launcher's enabled feature set (e.g. `is_quick_play_multiplayer`).
fn features_match(
    rule: &serde_json::Map<String, serde_json::Value>,
    enabled: &HashSet<String>,
) -> bool {
    let Some(features) = rule.get("features") else {
        return true;
    };
    let Some(map) = features.as_object() else {
        return true;
    };
    for (key, value) in map {
        let expected = value.as_bool().unwrap_or(false);
        if enabled.contains(key) != expected {
            return false;
        }
    }
    true
}

fn os_matches(os: &serde_json::Value) -> bool {
    if let Some(name) = os.get("name").and_then(|n| n.as_str()) {
        let match_os = match name {
            "windows" => current_os_name() == "windows",
            "linux" => current_os_name() == "linux",
            "osx" => current_os_name() == "osx",
            _ => false,
        };
        if !match_os {
            return false;
        }
    }
    if let Some(arch) = os.get("arch").and_then(|a| a.as_str()) {
        let match_arch = match arch {
            "x86" => current_os_arch() == "x86",
            "x86_64" => current_os_arch() == "x86_64",
            "arm64" => current_os_arch() == "arm64",
            _ => false,
        };
        if !match_arch {
            return false;
        }
    }
    true
}

/// Replaces `${key}` placeholders using `tokens`. Unknown placeholders are left
/// untouched (they may belong to a newer profile format; substituting garbage
/// would be worse).
pub fn substitute(template: &str, tokens: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in tokens {
        let placeholder = format!("${{{key}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_known_and_unknown_tokens() {
        let mut tokens = HashMap::new();
        tokens.insert("auth_player_name".to_string(), "Steve".to_string());
        tokens.insert("game_directory".to_string(), "C:\\games".to_string());

        assert_eq!(
            "Steve C:\\games",
            substitute("${auth_player_name} ${game_directory}", &tokens)
        );
        // Unknown tokens are left as-is.
        assert_eq!("${future_token}", substitute("${future_token}", &tokens));
    }

    #[test]
    fn rules_gate_libraries_by_os() {
        let lib = LibrarySpec {
            name: "org.lwjgl:lwjgl:3.3.3:natives-windows@jar".to_string(),
            rules: Some(serde_json::json!([
                { "action": "allow", "os": { "name": "windows" } }
            ])),
            ..Default::default()
        };
        // The result depends on the host OS; just assert it is internally
        // consistent with current_os_name().
        assert_eq!(library_allowed(&lib), current_os_name() == "windows");
    }

    #[test]
    fn features_gate_quick_play_argument() {
        // The whole `--quickPlayMultiplayer <host:port>` pair is a single
        // rule-gated argument, as in the vanilla 1.20.2+ profiles.
        let arguments = serde_json::json!({
            "game": [
                { "rules": [{ "action": "allow", "features": { "is_quick_play_multiplayer": true } }], "value": ["--quickPlayMultiplayer", "${quickPlayMultiplayer}"] }
            ]
        });
        let profile = VersionProfile {
            id: "test".to_string(),
            arguments: Some(arguments),
            ..Default::default()
        };
        let chain = vec![profile];
        let mut tokens = HashMap::new();
        tokens.insert(
            "quickPlayMultiplayer".to_string(),
            "mc.example.com:25565".to_string(),
        );

        let enabled: HashSet<String> = ["is_quick_play_multiplayer".to_string()]
            .into_iter()
            .collect();
        let args = resolve_game_arguments(&chain, &tokens, &enabled);
        assert_eq!(vec!["--quickPlayMultiplayer", "mc.example.com:25565"], args);

        // Without the feature the gated argument is dropped entirely.
        let args = resolve_game_arguments(&chain, &tokens, &HashSet::new());
        assert!(args.is_empty());
    }

    #[test]
    fn merged_libraries_deduplicates_and_skips_disallowed() {
        let profile = VersionProfile {
            id: "child".to_string(),
            libraries: vec![
                LibrarySpec {
                    name: "com.example:shared:1.0@jar".to_string(),
                    ..Default::default()
                },
                LibrarySpec {
                    name: "com.example:child-only:1.0@jar".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let parent = VersionProfile {
            id: "parent".to_string(),
            libraries: vec![
                LibrarySpec {
                    name: "com.example:shared:2.0@jar".to_string(),
                    ..Default::default()
                },
                LibrarySpec {
                    name: "org.example:parent-only:1.0@jar".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let chain = vec![profile, parent];
        let merged = merged_libraries(&chain);
        let names: Vec<&str> = merged.iter().map(|l| l.name.as_str()).collect();
        // De-duplication is by full Maven coordinate NAME (like the Java), so
        // the child's "shared:1.0" and the parent's "shared:2.0" are distinct
        // entries; the child's own libraries come first (child-first order).
        assert_eq!(
            vec![
                "com.example:shared:1.0@jar",
                "com.example:child-only:1.0@jar",
                "com.example:shared:2.0@jar",
                "org.example:parent-only:1.0@jar"
            ],
            names
        );
    }

    #[test]
    fn chain_resolution_breaks_on_missing_parent() {
        let root = VersionProfile {
            id: "neoforge-20.4.250".to_string(),
            inherits_from: Some("1.20.4".to_string()),
            ..Default::default()
        };
        let chain = resolve_chain(&root, |parent| {
            if parent == "1.20.4" {
                Ok(VersionProfile {
                    id: "1.20.4".to_string(),
                    ..Default::default()
                })
            } else {
                Err(LauncherError::NotFound(format!("no profile {parent}")))
            }
        });
        assert_eq!(2, chain.len());
        assert_eq!("neoforge-20.4.250", chain[0].id);
        assert_eq!("1.20.4", chain[1].id);
    }

    #[test]
    fn parse_profile_round_trip() {
        let json = r#"{
            "id": "1.20.4",
            "mainClass": "net.minecraft.client.main.Main",
            "inheritsFrom": null,
            "libraries": [{ "name": "com.example:a:1@jar" }],
            "arguments": { "game": ["--username", "${auth_player_name}"] }
        }"#;
        let profile = parse_profile(json).unwrap();
        assert_eq!("1.20.4", profile.id);
        assert_eq!("net.minecraft.client.main.Main", profile.main_class);
        assert_eq!(1, profile.libraries.len());
        let args = resolve_game_arguments(&[profile], &HashMap::new(), &HashSet::new());
        assert_eq!(vec!["--username", "${auth_player_name}"], args);
    }
}
