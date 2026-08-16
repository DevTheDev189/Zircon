//! Options-file utilities shared by the launch pipeline: line upserting for
//! Minecraft-style key/value files and application of the player's local
//! shaderpack/resourcepack selection right before a launch.
//!
//! `options.txt` uses `key:value` lines, `config/iris.properties` uses
//! `key=value` — the separator is simply part of the upsert prefix.
//!
//! Port of `com.mcmanager.client.launch.OptionsFileUtil` and
//! `com.mcmanager.client.launch.PackOptionsWriter`.

use std::path::Path;

use serde::Deserialize;

/// Upserts a single `prefix + value` line in a Minecraft-style options file,
/// preserving every other line. The separator (`:` or `=`) is part of
/// `key_prefix`, so `options.txt` and `iris.properties` share one code path.
///
/// Port of `com.mcmanager.client.launch.OptionsFileUtil`.
pub struct OptionsFileUtil;

impl OptionsFileUtil {
    /// Replaces every line starting with `key_prefix` (or appends one when no
    /// line matches) and rewrites the file. Mirrors the Java
    /// `readAllLines`/`Files.write` behavior: a missing file starts empty,
    /// unrelated lines are preserved, and the file always ends with a trailing
    /// newline.
    pub fn upsert_line(file: &Path, key_prefix: &str, value: &str) -> std::io::Result<()> {
        let mut lines: Vec<String> = if file.is_file() {
            // Java decodes with a replacement decoder, so lossy decoding is the
            // faithful choice here.
            String::from_utf8_lossy(&std::fs::read(file)?)
                .lines()
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };

        let new_line = format!("{key_prefix}{value}");
        let mut found = false;
        for line in lines.iter_mut() {
            if line.starts_with(key_prefix) {
                *line = new_line.clone();
                found = true;
            }
        }
        if !found {
            lines.push(new_line);
        }

        // Java `Files.write(path, lines, UTF_8)` terminates every line, so the
        // file always ends with a newline.
        let mut content = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                content.push('\n');
            }
            content.push_str(line);
        }
        content.push('\n');
        std::fs::write(file, content)
    }
}

/// Applies the player's local pack selection to the game directory right
/// before launch — never the server's full synced set, only what the player
/// explicitly opted into. Self-contained: takes just `game_dir` and loads the
/// selection itself, so the runner needs no extra parameters.
///
/// Port of `com.mcmanager.client.launch.PackOptionsWriter`.
pub struct PackOptionsWriter;

/// The player's local, per-instance pack selection persisted as
/// `pack-selection.json` in the game directory. Mirrors the Gson shape of
/// `com.mcmanager.client.pack.PackSelection`; unknown/malformed files fall
/// back to the empty selection, exactly like the Java silent catch.
#[derive(Default, Deserialize)]
#[serde(default)]
struct PackSelection {
    #[serde(rename = "shadersEnabled")]
    shaders_enabled: bool,
    #[serde(rename = "activeShaderpack")]
    active_shaderpack: Option<String>,
    #[serde(rename = "activeResourcepacks")]
    active_resourcepacks: Vec<String>,
}

impl PackSelection {
    fn load(game_dir: &Path) -> PackSelection {
        let file = game_dir.join("pack-selection.json");
        if !file.is_file() {
            return PackSelection::default();
        }
        match std::fs::read_to_string(&file) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(_) => PackSelection::default(),
        }
    }
}

impl PackOptionsWriter {
    /// Writes `config/iris.properties` (shader state) and the
    /// `options.txt` `resourcePacks` entry from the player's selection.
    pub fn apply(game_dir: &Path) -> std::io::Result<()> {
        let selection = PackSelection::load(game_dir);
        apply_shaderpack(game_dir, &selection)?;
        apply_resourcepacks(game_dir, &selection)?;
        Ok(())
    }
}

/// Writes `config/iris.properties` — the file both Iris and Oculus read for
/// their shader state — with only the pack the player selected, or disabled
/// entirely. Packs live in `shaderpacks/` as `.zip` files, which is exactly
/// the form Iris accepts for the `shaderPack` value.
fn apply_shaderpack(game_dir: &Path, selection: &PackSelection) -> std::io::Result<()> {
    let enabled = selection.shaders_enabled && selection.active_shaderpack.is_some();
    let pack = if enabled {
        selection.active_shaderpack.as_deref().unwrap_or("")
    } else {
        ""
    };
    let iris_options = game_dir.join("config").join("iris.properties");
    if let Some(parent) = iris_options.parent() {
        std::fs::create_dir_all(parent)?;
    }
    OptionsFileUtil::upsert_line(&iris_options, "enableShaders=", &enabled.to_string())?;
    OptionsFileUtil::upsert_line(&iris_options, "shaderPack=", pack)?;
    tracing::info!(
        "Shaders: enabled={}, pack={}",
        enabled,
        if enabled { pack } else { "(none)" }
    );
    Ok(())
}

/// Writes `options.txt`'s `resourcePacks` entry from the player's checked
/// packs, "vanilla" first.
fn apply_resourcepacks(game_dir: &Path, selection: &PackSelection) -> std::io::Result<()> {
    let options = game_dir.join("options.txt");
    let mut entries = vec!["\"vanilla\"".to_string()];
    for filename in &selection.active_resourcepacks {
        entries.push(format!("\"file/{filename}\""));
    }
    OptionsFileUtil::upsert_line(
        &options,
        "resourcePacks:",
        &format!("[{}]", entries.join(",")),
    )?;
    tracing::info!(
        "Texture packs active: [{}]",
        selection.active_resourcepacks.join(", ")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Unique scratch directory per test (no `tempfile` dependency available).
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zircon-options-test-{tag}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn upsert_line_preserves_unrelated_lines() {
        let dir = temp_dir("preserve");
        let file = dir.join("options.txt");
        std::fs::write(&file, "a:1\nb:2\n").unwrap();
        OptionsFileUtil::upsert_line(&file, "b:", "3").unwrap();
        assert_eq!("a:1\nb:3\n", std::fs::read_to_string(&file).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_line_replaces_existing_key() {
        let dir = temp_dir("replace");
        let file = dir.join("options.txt");
        std::fs::write(&file, "k:1\n").unwrap();
        OptionsFileUtil::upsert_line(&file, "k:", "9").unwrap();
        OptionsFileUtil::upsert_line(&file, "k:", "8").unwrap();
        assert_eq!("k:8\n", std::fs::read_to_string(&file).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_line_replaces_every_matching_line() {
        // The Java loop rewrites all lines starting with the prefix, not just
        // the first one.
        let dir = temp_dir("all-matches");
        let file = dir.join("options.txt");
        std::fs::write(&file, "k:1\nk:2\nother:3\n").unwrap();
        OptionsFileUtil::upsert_line(&file, "k:", "4").unwrap();
        assert_eq!(
            "k:4\nk:4\nother:3\n",
            std::fs::read_to_string(&file).unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_line_appends_when_missing() {
        let dir = temp_dir("append");
        let file = dir.join("options.txt");
        std::fs::write(&file, "a:1\n").unwrap();
        OptionsFileUtil::upsert_line(&file, "c:", "5").unwrap();
        assert_eq!("a:1\nc:5\n", std::fs::read_to_string(&file).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_line_handles_empty_and_missing_files() {
        let dir = temp_dir("empty");
        let empty = dir.join("empty.txt");
        std::fs::write(&empty, "").unwrap();
        OptionsFileUtil::upsert_line(&empty, "key:", "value").unwrap();
        assert_eq!("key:value\n", std::fs::read_to_string(&empty).unwrap());

        let missing = dir.join("missing.txt");
        OptionsFileUtil::upsert_line(&missing, "key:", "value").unwrap();
        assert_eq!("key:value\n", std::fs::read_to_string(&missing).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_line_supports_equals_separator_prefix() {
        let dir = temp_dir("iris");
        let file = dir.join("iris.properties");
        OptionsFileUtil::upsert_line(&file, "enableShaders=", "true").unwrap();
        OptionsFileUtil::upsert_line(&file, "shaderPack=", "BSL.zip").unwrap();
        assert_eq!(
            "enableShaders=true\nshaderPack=BSL.zip\n",
            std::fs::read_to_string(&file).unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pack_options_writer_applies_selection() {
        let dir = temp_dir("apply");
        std::fs::write(
            dir.join("pack-selection.json"),
            r#"{
                "shadersEnabled": true,
                "activeShaderpack": "BSL.zip",
                "activeResourcepacks": ["faithful.zip", "dramatic-sky.zip"]
            }"#,
        )
        .unwrap();

        PackOptionsWriter::apply(&dir).unwrap();

        let iris = std::fs::read_to_string(dir.join("config").join("iris.properties")).unwrap();
        assert_eq!("enableShaders=true\nshaderPack=BSL.zip\n", iris);

        let options = std::fs::read_to_string(dir.join("options.txt")).unwrap();
        assert_eq!(
            "resourcePacks:[\"vanilla\",\"file/faithful.zip\",\"file/dramatic-sky.zip\"]\n",
            options
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pack_options_writer_defaults_when_no_selection_file() {
        let dir = temp_dir("default");
        PackOptionsWriter::apply(&dir).unwrap();

        let iris = std::fs::read_to_string(dir.join("config").join("iris.properties")).unwrap();
        assert_eq!("enableShaders=false\nshaderPack=\n", iris);

        let options = std::fs::read_to_string(dir.join("options.txt")).unwrap();
        assert_eq!("resourcePacks:[\"vanilla\"]\n", options);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pack_options_writer_disables_shaders_when_pack_missing_or_disabled() {
        let dir = temp_dir("disabled");
        // Explicitly disabled.
        std::fs::write(
            dir.join("pack-selection.json"),
            r#"{"shadersEnabled": false, "activeShaderpack": "BSL.zip"}"#,
        )
        .unwrap();
        PackOptionsWriter::apply(&dir).unwrap();
        let iris = std::fs::read_to_string(dir.join("config").join("iris.properties")).unwrap();
        assert_eq!("enableShaders=false\nshaderPack=\n", iris);

        // Enabled but no active pack: still disabled, shaderPack empty.
        std::fs::write(
            dir.join("pack-selection.json"),
            r#"{"shadersEnabled": true}"#,
        )
        .unwrap();
        PackOptionsWriter::apply(&dir).unwrap();
        let iris = std::fs::read_to_string(dir.join("config").join("iris.properties")).unwrap();
        assert_eq!("enableShaders=false\nshaderPack=\n", iris);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
