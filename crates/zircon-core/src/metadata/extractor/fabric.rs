use serde_json::Value;
use crate::model::{ModLoaderType, ModMetadata};
use super::MetadataError;

pub(crate) fn text(root: &Value, key: &str) -> Option<String> {
    root.get(key)
        .and_then(|el| el.as_str())
        .map(|s| s.to_string())
}

pub fn parse_fabric_metadata(content: &str) -> Result<ModMetadata, MetadataError> {
    let root: Value = serde_json::from_str(content)
        .map_err(|e| MetadataError::Invalid(format!("Invalid fabric.mod.json: {e}")))?;
    let obj = root.as_object().ok_or_else(|| {
        MetadataError::Invalid("Invalid fabric.mod.json: root is not an object".to_string())
    })?;

    let id = text(&root, "id");
    let id = match id {
        Some(id) if !id.trim().is_empty() => id,
        _ => {
            return Err(MetadataError::Invalid(
                "fabric.mod.json is missing required field 'id'".to_string(),
            ))
        }
    };
    let name = text(&root, "name")
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let version = text(&root, "version").unwrap_or_else(|| "0.0.0".to_string());
    let description = text(&root, "description").unwrap_or_default();
    let author = fabric_authors(&root);

    Ok(ModMetadata::new(
        id,
        name,
        version,
        description,
        author,
        ModLoaderType::Fabric,
        fabric_environment(obj),
    ))
}

/// `authors` is either a string or an array of strings / `{"name": ...}`
/// objects. Joins multiple authors with ", ".
pub fn fabric_authors(root: &Value) -> String {
    let Some(authors) = root.get("authors") else {
        return String::new();
    };
    if let Some(arr) = authors.as_array() {
        arr.iter()
            .filter_map(|a| {
                if let Some(s) = a.as_str() {
                    Some(s.to_string())
                } else if let Some(obj) = a.as_object() {
                    obj.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else if let Some(s) = authors.as_str() {
        s.to_string()
    } else {
        String::new()
    }
}

/// `environment` is a string (`"*"`, `"client"`, `"server"`) in the current
/// schema, but very old jars used an object like `{"client": "*"}`.
pub fn fabric_environment(obj: &serde_json::Map<String, Value>) -> String {
    let env = match obj.get("environment") {
        Some(env) => env,
        None => return "*".to_string(),
    };
    if let Some(env_obj) = env.as_object() {
        if env_obj.contains_key("client") && env_obj.contains_key("server") {
            return "both".to_string();
        }
        if env_obj.contains_key("client") {
            return "client".to_string();
        }
        if env_obj.contains_key("server") {
            return "server".to_string();
        }
        return "*".to_string();
    }
    env.as_str().unwrap_or("*").to_string()
}

