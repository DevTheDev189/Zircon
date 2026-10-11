use std::io::Read;
use serde_json::Value;

pub fn fabric_icon_candidates(content: &str, mod_id: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(root) = serde_json::from_str::<Value>(content) {
        if let Some(icon) = root.get("icon") {
            if let Some(path) = icon.as_str() {
                candidates.push(path.to_string());
                candidates.push(format!("assets/{mod_id}/{path}"));
            } else if let Some(obj) = icon.as_object() {
                for (_k, v) in obj {
                    if let Some(path) = v.as_str() {
                        candidates.push(path.to_string());
                        candidates.push(format!("assets/{mod_id}/{path}"));
                    }
                }
            }
        }
    }
    candidates.push(format!("assets/{mod_id}/icon.png"));
    candidates.push("icon.png".to_string());
    candidates
}

pub fn toml_icon_candidates(content: &str, mod_id: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(value) = toml::from_str::<toml::Value>(content) {
        if let Some(mods) = value.get("mods").and_then(|v| v.as_array()) {
            for m in mods {
                if let Some(logo) = m.get("logoFile").and_then(|v| v.as_str()) {
                    candidates.push(logo.to_string());
                    candidates.push(format!("assets/{mod_id}/{logo}"));
                }
            }
        }
    }
    candidates.push(format!("assets/{mod_id}/icon.png"));
    candidates.push("icon.png".to_string());
    candidates
}

pub fn extract_icon_from_zip<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, candidates: &[String]) -> Option<String> {
    use base64::Engine as _;
    const MAX_ICON_BYTES: u64 = 512 * 1024;
    for candidate in candidates {
        let clean = candidate.trim().trim_start_matches('/');
        if clean.is_empty() {
            continue;
        }
        if let Ok(mut entry) = zip.by_name(clean) {
            if entry.size() > MAX_ICON_BYTES || entry.size() == 0 {
                continue;
            }
            let mut bytes = Vec::new();
            if entry.by_ref().take(MAX_ICON_BYTES).read_to_end(&mut bytes).is_ok() && !bytes.is_empty() {
                let lower = clean.to_ascii_lowercase();
                let mime = if lower.ends_with(".png") {
                    "image/png"
                } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
                    "image/jpeg"
                } else if lower.ends_with(".webp") {
                    "image/webp"
                } else if lower.ends_with(".svg") {
                    "image/svg+xml"
                } else {
                    "image/png"
                };
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Some(format!("data:{mime};base64,{b64}"));
            }
        }
    }
    None
}

