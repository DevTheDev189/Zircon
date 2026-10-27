use serde_json::Value;
use crate::model::{ModLoaderType, ModMetadata};
use super::{MetadataError, FORGE_ENTRY, NEOFORGE_ENTRY};

pub fn parse_mcmod_info(content: &str) -> Result<ModMetadata, MetadataError> {
    let root: Value = serde_json::from_str(content)
        .map_err(|e| MetadataError::Invalid(format!("Malformed mcmod.info: {e}")))?;

    let entry = if let Some(arr) = root.as_array() {
        arr.first()
    } else if let Some(arr) = root.get("modList").and_then(|v| v.as_array()) {
        arr.first()
    } else {
        None
    };

    let obj = entry
        .and_then(|v| v.as_object())
        .ok_or_else(|| MetadataError::Invalid("mcmod.info contains no mod definitions".to_string()))?;

    let id = obj
        .get("modid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if id.is_empty() {
        return Err(MetadataError::Invalid("mcmod.info missing 'modid'".to_string()));
    }

    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(id)
        .trim();

    let version = obj
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .trim();

    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let author = if let Some(authors) = obj.get("authorList").and_then(|v| v.as_array()) {
        authors
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    } else if let Some(authors) = obj.get("authors").and_then(|v| v.as_array()) {
        authors
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        String::new()
    };

    Ok(ModMetadata::new(
        id,
        name,
        version,
        description,
        author,
        ModLoaderType::Forge,
        "*",
    ))
}



pub fn parse_toml_metadata(
    content: &str,
    loader_type: ModLoaderType,
) -> Result<ModMetadata, MetadataError> {
    let value: toml::Value = toml::from_str(content)
        .map_err(|e| MetadataError::Invalid(format!("Invalid TOML metadata: {e}")))?;

    let entry_name = match loader_type {
        ModLoaderType::NeoForge => NEOFORGE_ENTRY,
        _ => FORGE_ENTRY,
    };

    let mods = value
        .get("mods")
        .and_then(|v| v.as_array())
        .filter(|a| !a.is_empty())
        .ok_or_else(|| {
            MetadataError::Invalid(format!("Missing [[mods]] section in {entry_name}"))
        })?;

    let first = mods
        .first()
        .and_then(|v| v.as_table())
        .ok_or_else(|| MetadataError::Invalid(format!("Empty [[mods]] entry in {entry_name}")))?;

    let id = first
        .get("modId")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            MetadataError::Invalid(format!("Missing 'modId' in [[mods]] entry of {entry_name}"))
        })?;

    let name = first
        .get("displayName")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| id.clone());
    // TOML is weakly typed: version is usually a string but occasionally a number.
    let version =
        toml_string_or_number(first.get("version")).unwrap_or_else(|| "0.0.0".to_string());
    let description = first
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let author = first
        .get("authors")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let environment = if first.get("clientSideOnly").and_then(|v| v.as_bool()).unwrap_or(false) {
        "client"
    } else if let Some(display_test) = first.get("displayTest").and_then(|v| v.as_str()) {
        let dt = display_test.to_ascii_uppercase();
        if dt.contains("IGNORE_SERVER_VERSION") || dt.contains("IGNORE_ALL_VERSION") {
            "client"
        } else {
            "both"
        }
    } else if let Some(side) = first.get("side").and_then(|v| v.as_str()) {
        match side.to_ascii_lowercase().as_str() {
            "client" => "client",
            "server" => "server",
            _ => "both",
        }
    } else {
        "both"
    };

    Ok(ModMetadata::new(
        id,
        name,
        version,
        description,
        author,
        loader_type,
        environment,
    ))
}

pub fn toml_string_or_number(value: Option<&toml::Value>) -> Option<String> {
    match value {
        Some(toml::Value::String(s)) => Some(s.clone()),
        Some(toml::Value::Integer(i)) => Some(i.to_string()),
        Some(toml::Value::Float(f)) => Some(f.to_string()),
        _ => None,
    }
}

