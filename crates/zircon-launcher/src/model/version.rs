//! A Minecraft version profile JSON (as written by the Forge/NeoForge
//! installers into `versions/<id>/<id>.json`, and by Mojang for vanilla).
//! Models only the fields the launcher needs to build a launch command.
//!
//! ```json
//! {
//!   "id": "neoforge-20.4.250",
//!   "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
//!   "inheritsFrom": "1.20.4",
//!   "arguments": { "game": [...], "jvm": [...] },
//!   "libraries": [ { "name": "group:artifact:version@jar", "downloads": {...} } ]
//! }
//! ```
//!
//! Unknown fields are ignored, so vanilla and loader profiles parse with the
//! same model. `rules` and `downloads` stay as raw JSON (`serde_json::Value`)
//! because their shape varies between profiles — the resolver logic in
//! `crate::launch::profile` interprets them.
//!
//! Port of `com.mcmanager.client.profile.VersionProfile` and
//! `com.mcmanager.client.profile.LibrarySpec`.

use serde::Deserialize;

/// One version profile (vanilla or loader-generated).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionProfile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub main_class: String,
    /// The id of the parent profile (usually the vanilla Minecraft version).
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub libraries: Vec<LibrarySpec>,
    /// `{"jvm": [...], "game": [...]}`; entries are strings or rule objects.
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
    /// Legacy `minecraftArguments` string profiles, pre-1.13.
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
}

/// One entry in a version profile's `libraries` array.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LibrarySpec {
    /// Maven coordinate, e.g. `net.neoforged:neoforge:20.4.250@jar`.
    pub name: String,
    /// Optional `rules` array restricting the library to certain OSes.
    #[serde(default)]
    pub rules: Option<serde_json::Value>,
    /// Optional `downloads` object (`artifact.path/url` and `classifiers`).
    #[serde(default)]
    pub downloads: Option<serde_json::Value>,
}

/// Parsed Maven coordinate: `group:artifact:version[:classifier]@extension`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactCoordinates {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl LibrarySpec {
    /// Parses a Maven coordinate of the form `g:a:v[:c]@ext`.
    pub fn parse_coordinates(name: &str) -> Option<ArtifactCoordinates> {
        if name.is_empty() {
            return None;
        }
        let at = name.find('@');
        let (extension, coords) = match at {
            Some(idx) => (name[idx + 1..].to_string(), &name[..idx]),
            None => ("jar".to_string(), name),
        };
        let parts: Vec<&str> = coords.split(':').collect();
        if parts.len() < 3 {
            return None;
        }
        Some(ArtifactCoordinates {
            group: parts[0].to_string(),
            artifact: parts[1].to_string(),
            version: parts[2].to_string(),
            classifier: parts.get(3).map(|c| c.to_string()),
            extension,
        })
    }

    /// The relative path of this library under the libraries directory, e.g.
    /// `net/neoforged/neoforge/20.4.250/neoforge-20.4.250-client.jar`.
    /// Prefers the installer-provided `downloads.artifact.path` and falls back
    /// to deriving the path from the Maven coordinate.
    pub fn artifact_path(&self) -> Option<String> {
        if let Some(downloads) = self.downloads.as_ref() {
            if let Some(path) = downloads
                .get("artifact")
                .and_then(|a| a.get("path"))
                .and_then(|p| p.as_str())
            {
                return Some(path.to_string());
            }
        }
        let coords = Self::parse_coordinates(&self.name)?;
        let group_path = coords.group.replace('.', "/");
        let mut file = format!("{}-{}", coords.artifact, coords.version);
        if let Some(classifier) = &coords.classifier {
            file.push('-');
            file.push_str(classifier);
        }
        file.push('.');
        file.push_str(&coords.extension);
        Some(format!(
            "{group_path}/{}/{}/{}",
            coords.artifact, coords.version, file
        ))
    }

    /// The download URL for this library, or `None` when the profile has none.
    pub fn download_url(&self) -> Option<String> {
        self.downloads
            .as_ref()?
            .get("artifact")?
            .get("url")?
            .as_str()
            .map(str::to_string)
    }

    /// `group:artifact` key used to de-duplicate shared dependencies
    /// (a loader-provided version replaces the vanilla one).
    pub fn group_artifact(&self) -> Option<String> {
        let coords = Self::parse_coordinates(&self.name)?;
        Some(format!("{}:{}", coords.group, coords.artifact))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_coordinate_parsing() {
        let coords = LibrarySpec::parse_coordinates("net.neoforged:neoforge:20.4.250@jar")
            .expect("coordinates");
        assert_eq!("net.neoforged", coords.group);
        assert_eq!("neoforge", coords.artifact);
        assert_eq!("20.4.250", coords.version);
        assert_eq!(None, coords.classifier);
        assert_eq!("jar", coords.extension);

        let coords =
            LibrarySpec::parse_coordinates("net.neoforged:mergetool:2.0.0:api@jar").unwrap();
        assert_eq!(Some("api".to_string()), coords.classifier);

        assert!(LibrarySpec::parse_coordinates("not-a-coordinate").is_none());
    }

    #[test]
    fn artifact_path_derived_from_coordinate() {
        let lib = LibrarySpec {
            name: "net.neoforged:neoforge:20.4.250@jar".to_string(),
            ..Default::default()
        };
        assert_eq!(
            Some("net/neoforged/neoforge/20.4.250/neoforge-20.4.250.jar".to_string()),
            lib.artifact_path()
        );

        let lib = LibrarySpec {
            name: "org.lwjgl:lwjgl:3.3.3:natives-windows@jar".to_string(),
            ..Default::default()
        };
        assert_eq!(
            Some("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar".to_string()),
            lib.artifact_path()
        );
    }

    #[test]
    fn artifact_path_prefers_downloads_entry() {
        let lib = LibrarySpec {
            name: "g:a:1@jar".to_string(),
            downloads: Some(serde_json::json!({
                "artifact": { "path": "custom/dir/lib.jar", "url": "https://x/lib.jar" }
            })),
            ..Default::default()
        };
        assert_eq!(Some("custom/dir/lib.jar".to_string()), lib.artifact_path());
        assert_eq!(Some("https://x/lib.jar".to_string()), lib.download_url());
    }
}
