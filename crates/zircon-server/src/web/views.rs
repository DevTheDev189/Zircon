//! View mappers: Java's `toMap()` helpers for the admin web UI. These produce
//! the exact response shapes the bundled SPA consumes (including the defaults
//! Java applied, e.g. `title` falling back to the file name).

use zircon_core::model::{BackupEntry, InstanceConfig, ModEntry, PackEntry};

use crate::process::player_tracker::PlayerHistoryEntry;

/// Plain map view of a `ModEntry` for the admin web UI.
pub fn mod_entry_to_map(entry: &ModEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id,
        "filename": entry.filename,
        "sha1": entry.sha1,
        "murmur3": entry.murmur3,
        "origin": entry.origin,
        "downloadUrl": entry.download_url,
        "fileSize": entry.file_size,
        "title": entry.display_title(),
        "description": entry.description.clone().unwrap_or_default(),
        "iconUrl": entry.icon_url.clone().unwrap_or_default(),
        "author": entry.author.clone().unwrap_or_default(),
        "compatible": entry.compatible,
        "warningMessage": entry.warning_message.clone().unwrap_or_default(),
    })
}

/// Plain map view of a `PackEntry` for the admin web UI.
pub fn pack_entry_to_map(entry: &PackEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id,
        "filename": entry.filename,
        "sha1": entry.sha1,
        "murmur3": entry.murmur3,
        "origin": entry.origin,
        "downloadUrl": entry.download_url,
        "fileSize": entry.file_size,
        "title": entry.display_title(),
        "iconUrl": entry.icon_url.clone().unwrap_or_default(),
    })
}

/// Plain map view of a `PlayerHistoryEntry` for the admin web UI.
pub fn player_history_to_map(entry: &PlayerHistoryEntry) -> serde_json::Value {
    serde_json::json!({
        "name": entry.name,
        "firstJoined": entry.first_joined,
        "lastJoined": entry.last_joined,
        "joinCount": entry.join_count,
    })
}

/// Plain map view of a `BackupEntry` for the admin web UI.
pub fn backup_entry_to_map(entry: &BackupEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id,
        "instanceId": entry.instance_id,
        "filename": entry.filename,
        "timestamp": entry.timestamp,
        "sizeBytes": entry.size_bytes,
        "triggerType": entry.trigger_type,
        "status": entry.status,
        "logs": entry.logs,
    })
}

/// Plain map view of an `InstanceConfig` for the admin web UI (includes live
/// runtime state).
pub fn instance_to_map(
    instance: &InstanceConfig,
    running: bool,
    player_count: usize,
    online_players: Vec<String>,
) -> serde_json::Value {
    serde_json::json!({
        "id": instance.id,
        "name": instance.name,
        "minecraftVersion": instance.minecraft_version,
        "modLoader": {
            "type": instance.loader_type(),
            "version": instance.loader_version(),
        },
        "internalMcPort": instance.internal_mc_port,
        "externalPort": instance.external_mc_port,
        "javaArgs": instance.java_args,
        "autoStart": instance.auto_start,
        "backupFrequency": instance.backup_frequency,
        "backupTime": instance.backup_time,
        "backupRetention": instance.backup_retention,
        "idleShutdownEnabled": instance.idle_shutdown_enabled,
        "idleShutdownMinutes": instance.idle_shutdown_minutes,
        "lastShutdownReason": instance.last_shutdown_reason,
        "wakeable": !running && instance.wakeable(),
        "running": running,
        "playerCount": player_count,
        "onlinePlayers": online_players,
    })
}

/// View of a Modrinth version for the version picker.
pub fn modrinth_version_to_map(
    version: &zircon_core::api::modrinth::ModrinthVersion,
) -> serde_json::Value {
    let file = version.primary_file().map(|f| {
        serde_json::json!({
            "url": f.url,
            "filename": f.filename,
            "sha1": f.sha1(),
            "size": f.size,
            "primary": f.primary,
        })
    });
    serde_json::json!({
        "id": version.id,
        "projectId": version.project_id,
        "name": version.name,
        "versionNumber": version.version_number,
        "gameVersions": version.game_versions,
        "loaders": version.loaders,
        "url": version.url,
        "file": file,
    })
}

/// View of a Modrinth search hit.
pub fn modrinth_hit_to_map(
    hit: &zircon_core::api::modrinth::ModrinthSearchHit,
) -> serde_json::Value {
    serde_json::json!({
        "projectId": hit.project_id,
        "slug": hit.slug,
        "title": hit.title,
        "description": hit.description,
        "author": hit.author,
        "downloads": hit.downloads,
        "iconUrl": hit.icon_url,
        "versions": hit.versions,
    })
}

/// View of a CurseForge mod search hit.
pub fn curseforge_mod_to_map(
    mod_entry: &zircon_core::api::curseforge::CurseForgeMod,
) -> serde_json::Value {
    serde_json::json!({
        "id": mod_entry.id,
        "name": mod_entry.name,
        "slug": mod_entry.slug,
        "summary": mod_entry.summary,
        "downloadCount": mod_entry.download_count,
        "gameVersions": mod_entry.game_versions,
        "websiteUrl": mod_entry.links.as_ref().map(|l| l.website_url.clone()),
    })
}

/// View of a CurseForge file.
pub fn curseforge_file_to_map(
    file: &zircon_core::api::curseforge::CurseForgeFile,
) -> serde_json::Value {
    serde_json::json!({
        "id": file.id,
        "displayName": file.display_name,
        "fileName": file.file_name,
        "downloadUrl": file.download_url,
        "fileFingerprint": file.file_fingerprint,
        "length": file.length,
    })
}

/// Parses a vanilla JSON player list entry (whitelist.json, ops.json,
/// banned-players.json) into the map shape the admin UI expects.
pub fn vanilla_player_entry_to_map(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("uuid".to_string(), str_or_empty(obj.get("uuid")));
    map.insert("name".to_string(), str_or_empty(obj.get("name")));
    for key in ["reason", "source", "created", "expires"] {
        if obj.contains_key(key) {
            map.insert(key.to_string(), str_or_empty(obj.get(key)));
        }
    }
    serde_json::Value::Object(map)
}

fn str_or_empty(value: Option<&serde_json::Value>) -> serde_json::Value {
    match value {
        Some(v) if v.is_string() => v.clone(),
        _ => serde_json::Value::String(String::new()),
    }
}
