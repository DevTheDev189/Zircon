//! Builds and runs the Minecraft client process with the resolved classpath,
//! injecting the session and auto-connecting the player straight to the server
//! (online path), or launching a local single-player session (offline path).
//!
//! Both paths share a pure command builder,
//! [`MinecraftRunner::build_launch_command`], that performs session
//! enforcement, token substitution and quick-play deduplication without any
//! process or file side effects; the async entry points layer the options-file
//! prep and the tokio process spawn on top of it.
//!
//! Port of `com.mcmanager.client.launch.MinecraftRunner`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};

use super::classpath::LaunchData;
use super::options::{OptionsFileUtil, PackOptionsWriter};
use super::profile::substitute;
use crate::auth::session::SessionData;
use crate::error::LauncherError;
use crate::sync::mod_sync::HashVerifier;
use zircon_core::metadata::extractor::validate_mod_jar_structure;

/// Launches the Minecraft client as a child process.
///
/// Port of `com.mcmanager.client.launch.MinecraftRunner`.
pub struct MinecraftRunner;

impl MinecraftRunner {
    /// Launches the game with a genuine Microsoft session, auto-connecting the
    /// player to `server_ip:server_port` and starting the game in fullscreen.
    ///
    /// Before spawning, `options.txt` is prepped so the game skips the
    /// third-party multiplayer warning and opens fullscreen, and the player's
    /// local shaderpack/resourcepack selection is applied. Returns the child
    /// immediately (like the Java `Process`); stdout/stderr lines are pumped
    /// into `output` on background tasks.
    pub async fn launch(
        &self,
        data: &LaunchData,
        session: &SessionData,
        game_dir: &Path,
        server_ip: &str,
        server_port: i32,
        output: Option<Arc<dyn Fn(String) + Send + Sync>>,
    ) -> Result<Child, LauncherError> {
        validate_mods_dir(game_dir)?;
        let command = Self::build_launch_command(
            data,
            Some(session),
            None,
            None,
            game_dir,
            Some(server_ip),
            Some(server_port),
        )?;
        prepare_options(game_dir)?;
        let preview = command
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        tracing::info!(
            "Launching Minecraft: {} --quickPlayMultiplayer {}:{} ...",
            preview,
            server_ip,
            server_port
        );
        spawn_game(&command, game_dir, output)
    }

    /// Launches the game in offline (single-player) mode using a local username
    /// instead of a Microsoft session: no server connection is attempted and
    /// the game runs with a vanilla/loader offline session (accessToken `0`,
    /// userType `legacy`).
    pub async fn launch_offline(
        &self,
        data: &LaunchData,
        username: &str,
        java_args: &str,
        game_dir: &Path,
        output: Option<Arc<dyn Fn(String) + Send + Sync>>,
    ) -> Result<Child, LauncherError> {
        validate_mods_dir(game_dir)?;
        let player = if username.trim().is_empty() {
            "Player"
        } else {
            username.trim()
        };
        let command = Self::build_launch_command(
            data,
            None,
            Some(player),
            Some(java_args),
            game_dir,
            None,
            None,
        )?;
        prepare_options(game_dir)?;
        let preview = command
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        tracing::info!(
            "Launching Minecraft (offline) as '{}': {} ...",
            player,
            preview
        );
        spawn_game(&command, game_dir, output)
    }

    /// Pure command builder shared by both launch paths and unit tests: no
    /// process is spawned and no file is touched.
    ///
    /// * `session: Some` — online launch; auto-connect args are appended.
    /// * `session: None` + `username: Some` — offline launch; no server args.
    /// * both `None` — [`LauncherError::InvalidInput`].
    ///
    /// In online mode an absent `server_ip`/`server_port` falls back to an
    /// empty host and port `0`; the Tauri shell always supplies both.
    pub(crate) fn build_launch_command(
        data: &LaunchData,
        session: Option<&SessionData>,
        username: Option<&str>,
        java_args: Option<&str>,
        game_dir: &Path,
        server_ip: Option<&str>,
        server_port: Option<i32>,
    ) -> Result<Vec<String>, LauncherError> {
        // Validate server_ip syntax before building launch args: garbage hosts
        // would otherwise be passed to the game (and could inject flags or
        // URLs into the command line). Accepts domains, IP literals and
        // bracketed IPv6 (as produced by `servers::format_host`).
        if let Some(host) = server_ip {
            if !host.is_empty() {
                // `url::Host::parse` alone accepts `--username`-style strings
                // (it treats them as domains), which would let a hostile
                // address inject JVM/game flags — reject any leading dash.
                if host.starts_with('-') || url::Host::parse(host).is_err() {
                    return Err(LauncherError::InvalidInput(format!(
                        "Invalid server address: {host}"
                    )));
                }
            }
        }
        match session {
            Some(session) => build_online_command(data, session, game_dir, server_ip, server_port),
            None => match username {
                Some(name) => build_offline_command(data, name, java_args, game_dir),
                None => Err(LauncherError::InvalidInput(
                    "no Microsoft session and no offline username provided".to_string(),
                )),
            },
        }
    }
}

/// Re-validates every `.jar` in the instance `mods/` folder right before the
/// game spawns. The mod sync validates staged downloads, but a file could be
/// swapped after the sync (local tampering), come from an offline instance's
/// locally-managed mods, or predate the structural checks — nothing malformed
/// may reach the loader. See [`validate_mod_jar_structure`] for what is
/// checked.
fn validate_mods_dir(game_dir: &Path) -> Result<(), LauncherError> {
    let mods_dir = game_dir.join("mods");
    if !mods_dir.is_dir() {
        return Ok(());
    }
    let mut invalid: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !HashVerifier::is_mod_jar(&name) {
                continue;
            }
            if let Err(e) = validate_mod_jar_structure(&path) {
                tracing::warn!("Pre-launch JAR check failed for {name}: {e}");
                invalid.push(name);
            }
        }
    }
    if !invalid.is_empty() {
        return Err(LauncherError::InvalidInput(format!(
            "Refusing to launch: the following mods failed structural validation \
             (not a valid ZIP, implausible compression ratio, or missing mod \
             metadata): {}. Re-sync with the server or remove them from the \
             mods folder.",
            invalid.join(", ")
        )));
    }
    Ok(())
}

/// Builds the online launch command. Mirrors the Java `launch` argument
/// assembly, including the MSA-only session enforcement.
fn build_online_command(
    data: &LaunchData,
    session: &SessionData,
    game_dir: &Path,
    server_ip: Option<&str>,
    server_port: Option<i32>,
) -> Result<Vec<String>, LauncherError> {
    let host = server_ip.unwrap_or("");
    let port = server_port.unwrap_or(0);
    let user_type = session.user_type.as_str();
    let access_token = session.access_token.as_str();

    // Only genuine Microsoft sessions may launch the game. A dummy/missing
    // token or a non-msa userType means the session did not come from
    // Microsoft auth — refuse instead of falling back to a fake token.
    if user_type != "msa" || access_token.trim().is_empty() || access_token == "0" {
        return Err(LauncherError::Auth(
            "Refusing to launch: no valid Microsoft session. \
             Please sign in with your Microsoft account."
                .to_string(),
        ));
    }

    let java = java_executable(&data.java_home);
    let mut command = vec![java.display().to_string()];
    // Forge/NeoForge contribute JVM args from the version profile chain:
    // -p module path, --add-modules/--add-opens/--add-exports, -D system
    // properties such as -DlibraryDirectory and -DignoreList.
    command.extend(data.jvm_args.iter().cloned());
    command.push("-Xmx4G".to_string());
    command.push(format!(
        "-Djava.library.path={}",
        data.natives_dir.display()
    ));
    command.push("-cp".to_string());
    command.push(data.classpath.clone());
    command.push(data.main_class.clone());

    if !data.game_args.is_empty() {
        // Forge/NeoForge: the version profile (including the inherited vanilla
        // profile) already supplies the complete standard game arguments
        // (--username, --gameDir, --accessToken, ...). Resolve their
        // placeholders instead of re-adding them below.
        let tokens = online_tokens(data, session, game_dir, host, port);
        let mut profile_args: Vec<String> = data
            .game_args
            .iter()
            .map(|arg| substitute(arg, &tokens))
            .collect();
        // The profile may contribute --quickPlayMultiplayer; drop it so the
        // canonical auto-connect args below win (no duplicate keys).
        drop_quick_play_pairs(&mut profile_args);
        command.extend(profile_args);
    } else {
        push_arg(&mut command, "--username", &session.username);
        push_arg(&mut command, "--version", &data.version_name);
        push_arg(&mut command, "--gameDir", &game_dir.display().to_string());
        push_arg(
            &mut command,
            "--assetsDir",
            &data.assets_dir.display().to_string(),
        );
        push_arg(&mut command, "--assetIndex", &data.asset_index_id);
        push_arg(&mut command, "--uuid", &session.uuid);
        push_arg(&mut command, "--accessToken", access_token);
        push_arg(&mut command, "--userType", user_type);
        push_arg(&mut command, "--versionType", "release");
    }

    // Auto-connect: modern Minecraft (1.20.2+) replaced --server/--port with
    // --quickPlayMultiplayer <host:port> and ignores the old args. Passing
    // both keeps compatibility with older versions (they ignore the unknown
    // one).
    push_arg(&mut command, "--server", host);
    push_arg(&mut command, "--port", &port.to_string());
    push_arg(
        &mut command,
        "--quickPlayMultiplayer",
        &format!("{host}:{port}"),
    );

    // Start the game in fullscreen mode.
    command.push("--fullscreen".to_string());
    Ok(command)
}

/// Builds the offline launch command, mirroring the Java `launchOffline`
/// argument assembly (no server/port/quick-play/fullscreen flags).
fn build_offline_command(
    data: &LaunchData,
    username: &str,
    java_args: Option<&str>,
    game_dir: &Path,
) -> Result<Vec<String>, LauncherError> {
    let player_name = if username.trim().is_empty() {
        "Player"
    } else {
        username.trim()
    };

    let java = java_executable(&data.java_home);
    let mut command = vec![java.display().to_string()];
    command.extend(data.jvm_args.iter().cloned());
    append_jvm_memory_args(&mut command, java_args);
    command.push(format!(
        "-Djava.library.path={}",
        data.natives_dir.display()
    ));
    command.push("-cp".to_string());
    command.push(data.classpath.clone());
    command.push(data.main_class.clone());

    let uuid = offline_uuid(player_name);

    if !data.game_args.is_empty() {
        // Forge/NeoForge: substitute the version profile's game-argument
        // placeholders with offline credentials instead of a live session.
        let tokens = offline_tokens(data, player_name, &uuid, game_dir);
        let mut profile_args: Vec<String> = data
            .game_args
            .iter()
            .map(|arg| substitute(arg, &tokens))
            .collect();
        // Drop any quick-play multiplayer args so offline stays single-player.
        drop_quick_play_pairs(&mut profile_args);
        command.extend(profile_args);
    } else {
        push_arg(&mut command, "--username", player_name);
        push_arg(&mut command, "--version", &data.version_name);
        push_arg(&mut command, "--gameDir", &game_dir.display().to_string());
        push_arg(
            &mut command,
            "--assetsDir",
            &data.assets_dir.display().to_string(),
        );
        push_arg(&mut command, "--assetIndex", &data.asset_index_id);
        push_arg(&mut command, "--uuid", &uuid);
        push_arg(&mut command, "--accessToken", "0");
        push_arg(&mut command, "--userType", "legacy");
        push_arg(&mut command, "--versionType", "release");
    }
    Ok(command)
}

/// The java executable under `java_home/bin`, `java.exe` on Windows.
fn java_executable(java_home: &Path) -> PathBuf {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    java_home.join("bin").join(exe)
}

/// Appends JVM memory/extra args, defaulting to `-Xmx4G` when blank. Mirrors
/// the Java `addJvmMemoryArgs` (trim + whitespace split, blanks skipped).
fn append_jvm_memory_args(command: &mut Vec<String>, java_args: Option<&str>) {
    let args = match java_args {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => "-Xmx4G".to_string(),
    };
    command.extend(args.split_whitespace().map(str::to_string));
}

/// Deterministic v3 UUID for an offline player, derived from the
/// `OfflinePlayer:<name>` bytes.
fn offline_uuid(player_name: &str) -> String {
    uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_DNS,
        format!("OfflinePlayer:{player_name}").as_bytes(),
    )
    .to_string()
}

/// Token map for resolving the version profile's game-argument placeholders in
/// an online launch. Mirrors the tokens the official launcher fills in from the
/// authenticated session and the resolved paths.
fn online_tokens(
    data: &LaunchData,
    session: &SessionData,
    game_dir: &Path,
    host: &str,
    port: i32,
) -> HashMap<String, String> {
    let mut tokens = HashMap::new();
    tokens.insert("auth_player_name".to_string(), session.username.clone());
    tokens.insert("auth_uuid".to_string(), session.uuid.clone());
    tokens.insert(
        "auth_access_token".to_string(),
        session.access_token.clone(),
    );
    tokens.insert("auth_xuid".to_string(), String::new());
    tokens.insert("clientid".to_string(), String::new());
    tokens.insert("user_type".to_string(), session.user_type.clone());
    tokens.insert("version_type".to_string(), "release".to_string());
    tokens.insert("version_name".to_string(), data.version_name.clone());
    tokens.insert("game_directory".to_string(), game_dir.display().to_string());
    tokens.insert(
        "assets_root".to_string(),
        data.assets_dir.display().to_string(),
    );
    tokens.insert("assets_index_name".to_string(), data.asset_index_id.clone());
    tokens.insert("quickPlayMultiplayer".to_string(), format!("{host}:{port}"));
    tokens.insert("launcher_name".to_string(), "mcmanager".to_string());
    tokens.insert("launcher_version".to_string(), "1.0.0".to_string());
    tokens
}

/// Token map for the offline launch: accessToken `0`, userType `legacy`, and
/// an empty quick-play target so offline stays single-player.
fn offline_tokens(
    data: &LaunchData,
    player_name: &str,
    uuid: &str,
    game_dir: &Path,
) -> HashMap<String, String> {
    let mut tokens = HashMap::new();
    tokens.insert("auth_player_name".to_string(), player_name.to_string());
    tokens.insert("auth_uuid".to_string(), uuid.to_string());
    tokens.insert("auth_access_token".to_string(), "0".to_string());
    tokens.insert("auth_xuid".to_string(), String::new());
    tokens.insert("clientid".to_string(), String::new());
    tokens.insert("user_type".to_string(), "legacy".to_string());
    tokens.insert("version_type".to_string(), "release".to_string());
    tokens.insert("version_name".to_string(), data.version_name.clone());
    tokens.insert("game_directory".to_string(), game_dir.display().to_string());
    tokens.insert(
        "assets_root".to_string(),
        data.assets_dir.display().to_string(),
    );
    tokens.insert("assets_index_name".to_string(), data.asset_index_id.clone());
    tokens.insert("quickPlayMultiplayer".to_string(), String::new());
    tokens.insert("launcher_name".to_string(), "mcmanager".to_string());
    tokens.insert("launcher_version".to_string(), "1.0.0".to_string());
    tokens
}

/// Removes every `--quickPlayMultiplayer <value>` pair (mirrors the Java
/// remove-in-place loop, which also handles the value being the flag itself).
fn drop_quick_play_pairs(args: &mut Vec<String>) {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--quickPlayMultiplayer" && i + 1 < args.len() {
            args.remove(i + 1);
            args.remove(i);
        } else {
            i += 1;
        }
    }
}

fn push_arg(command: &mut Vec<String>, key: &str, value: &str) {
    command.push(key.to_string());
    command.push(value.to_string());
}

/// Options prep shared by both launch paths, run before spawning: pre-accept
/// the "multiplayer is third-party" disclaimer so the game auto-joins instead
/// of stopping at the warning screen, set the video setting the game actually
/// honors on boot so the window opens fullscreen, and apply the player's local
/// shaderpack/resourcepack choices (never the server's full synced set).
///
/// Note: the Java offline path only applies `PackOptionsWriter`; per the
/// porting spec this prep runs on both paths.
fn prepare_options(game_dir: &Path) -> Result<(), LauncherError> {
    set_options_entry(game_dir, "skipMultiplayerWarning", "true")?;
    set_options_entry(game_dir, "fullscreen", "true")?;
    PackOptionsWriter::apply(game_dir)?;
    Ok(())
}

/// Upserts a `key:value` entry in the instance's `options.txt`.
fn set_options_entry(game_dir: &Path, key: &str, value: &str) -> Result<(), LauncherError> {
    let options = game_dir.join("options.txt");
    OptionsFileUtil::upsert_line(&options, &format!("{key}:"), value)?;
    tracing::info!("Set {}:{} in {}", key, value, options.display());
    Ok(())
}

/// Spawns the game in `game_dir` with piped stdout/stderr and pumps both
/// streams into `output` on background tasks. Returns the child immediately,
/// like the Java `Process`.
fn spawn_game(
    command: &[String],
    game_dir: &Path,
    output: Option<Arc<dyn Fn(String) + Send + Sync>>,
) -> Result<Child, LauncherError> {
    let mut cmd = Command::new(&command[0]);
    // Untrusted mod code runs inside this JVM: scrub the environment so host
    // secrets (AWS_ACCESS_KEY_ID, GITHUB_TOKEN, ...) can never leak into the
    // game process. Keep only what the JVM needs to function.
    cmd.env_clear();
    cmd.envs(std::env::vars().filter(|(k, _)| {
        let upper = k.to_ascii_uppercase();
        matches!(
            upper.as_str(),
            "PATH" | "SYSTEMROOT" | "USERPROFILE" | "HOME" | "TMP" | "TEMP"
        )
    }));
    cmd.args(&command[1..])
        .current_dir(game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(LauncherError::from)?;

    if let Some(stdout) = child.stdout.take() {
        let cb = output.clone();
        tokio::spawn(async move {
            pump_stream(stdout, cb).await;
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let cb = output.clone();
        tokio::spawn(async move {
            pump_stream(stderr, cb).await;
        });
    }
    Ok(child)
}

/// Reads every line from a child-process stream into the output callback,
/// mirroring the Java `pump` loop: panics raised by the consumer are ignored,
/// and lines fall back to stdout when no consumer was supplied.
async fn pump_stream<R>(reader: R, output: Option<Arc<dyn Fn(String) + Send + Sync>>)
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(cb) = output.as_ref() {
                cb(line);
            } else {
                println!("{line}");
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch_data() -> LaunchData {
        LaunchData {
            main_class: "net.minecraft.client.main.Main".to_string(),
            classpath: "/mc/libraries/*".to_string(),
            asset_index_id: "1.20.4".to_string(),
            version_name: "1.20.4".to_string(),
            assets_dir: PathBuf::from("/mc/assets"),
            natives_dir: PathBuf::from("/mc/natives"),
            java_home: PathBuf::from("/mc/jdk"),
            jvm_args: vec!["-Djava.rmi.server.hostname=127.0.0.1".to_string()],
            game_args: Vec::new(),
        }
    }

    fn session(user_type: &str, access_token: &str) -> SessionData {
        SessionData {
            access_token: access_token.to_string(),
            refresh_token: "refresh-token".to_string(),
            username: "Steve".to_string(),
            uuid: "5b39f8e0-9f0c-4f9a-9f0c-000000000001".to_string(),
            expires_at_millis: 0,
            user_type: user_type.to_string(),
        }
    }

    fn java_exe() -> &'static str {
        if cfg!(windows) {
            "java.exe"
        } else {
            "java"
        }
    }

    fn arg_value<'a>(command: &'a [String], key: &str) -> &'a str {
        let idx = command
            .iter()
            .position(|a| a == key)
            .unwrap_or_else(|| panic!("missing {key} in {command:?}"));
        &command[idx + 1]
    }

    #[test]
    fn online_profile_game_args_substitute_and_drop_quick_play() {
        let mut data = launch_data();
        data.game_args = vec![
            "--username".to_string(),
            "${auth_player_name}".to_string(),
            "--quickPlayMultiplayer".to_string(),
            "${quickPlayMultiplayer}".to_string(),
            "--assetsDir".to_string(),
            "${assets_root}".to_string(),
        ];
        let game_dir = Path::new("/game");
        let command = MinecraftRunner::build_launch_command(
            &data,
            Some(&session("msa", "tok123")),
            None,
            None,
            game_dir,
            Some("mc.example.com"),
            Some(25565),
        )
        .unwrap();

        assert_eq!(
            command[0],
            PathBuf::from("/mc/jdk")
                .join("bin")
                .join(java_exe())
                .display()
                .to_string()
        );
        // Profile placeholders resolved from the session/game-dir tokens.
        assert_eq!(arg_value(&command, "--username"), "Steve");
        assert_eq!(
            arg_value(&command, "--assetsDir"),
            data.assets_dir.display().to_string()
        );
        // Exactly one --quickPlayMultiplayer pair remains: the canonical one.
        let occurrences: Vec<usize> = command
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == "--quickPlayMultiplayer")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(1, occurrences.len());
        assert_eq!(command[occurrences[0] + 1], "mc.example.com:25565");
        assert_eq!(arg_value(&command, "--server"), "mc.example.com");
        assert_eq!(arg_value(&command, "--port"), "25565");
        assert_eq!(*command.last().unwrap(), "--fullscreen");
    }

    #[test]
    fn online_legacy_args_include_session_credentials() {
        let data = launch_data(); // game_args empty -> legacy flag path
        let game_dir = Path::new("/game");
        let command = MinecraftRunner::build_launch_command(
            &data,
            Some(&session("msa", "tok123")),
            None,
            None,
            game_dir,
            Some("mc.example.com"),
            Some(25565),
        )
        .unwrap();

        assert_eq!(arg_value(&command, "--username"), "Steve");
        assert_eq!(arg_value(&command, "--version"), "1.20.4");
        assert_eq!(
            arg_value(&command, "--gameDir"),
            game_dir.display().to_string()
        );
        assert_eq!(
            arg_value(&command, "--assetsDir"),
            data.assets_dir.display().to_string()
        );
        assert_eq!(arg_value(&command, "--assetIndex"), "1.20.4");
        assert_eq!(
            arg_value(&command, "--uuid"),
            "5b39f8e0-9f0c-4f9a-9f0c-000000000001"
        );
        assert_eq!(arg_value(&command, "--accessToken"), "tok123");
        assert_eq!(arg_value(&command, "--userType"), "msa");
        assert_eq!(arg_value(&command, "--versionType"), "release");
        assert_eq!(arg_value(&command, "--server"), "mc.example.com");
        assert_eq!(arg_value(&command, "--port"), "25565");
        assert_eq!(
            arg_value(&command, "--quickPlayMultiplayer"),
            "mc.example.com:25565"
        );
        assert_eq!(*command.last().unwrap(), "--fullscreen");
    }

    #[test]
    fn refuses_non_msa_or_invalid_sessions() {
        let data = launch_data();
        let game_dir = Path::new("/game");
        let expect_auth_error = |session: &SessionData| {
            MinecraftRunner::build_launch_command(
                &data,
                Some(session),
                None,
                None,
                game_dir,
                Some("mc.example.com"),
                Some(25565),
            )
            .unwrap_err()
        };

        // Dummy access token.
        let err = expect_auth_error(&session("msa", "0"));
        assert!(matches!(err, LauncherError::Auth(_)));
        // Blank access token.
        let err = expect_auth_error(&session("msa", "   "));
        assert!(matches!(err, LauncherError::Auth(_)));
        // Non-msa userType (e.g. legacy from a Mojang session).
        let err = expect_auth_error(&session("legacy", "tok123"));
        assert!(matches!(err, LauncherError::Auth(_)));

        // Neither a session nor an offline username is invalid input.
        let err =
            MinecraftRunner::build_launch_command(&data, None, None, None, game_dir, None, None)
                .unwrap_err();
        assert!(matches!(err, LauncherError::InvalidInput(_)));
    }

    #[test]
    fn offline_legacy_args_and_uuid_are_deterministic() {
        let data = launch_data();
        let game_dir = Path::new("/game");
        let first = MinecraftRunner::build_launch_command(
            &data,
            None,
            Some("Alex"),
            Some(""),
            game_dir,
            None,
            None,
        )
        .unwrap();
        let second = MinecraftRunner::build_launch_command(
            &data,
            None,
            Some("Alex"),
            Some(""),
            game_dir,
            None,
            None,
        )
        .unwrap();

        assert_eq!(arg_value(&first, "--username"), "Alex");
        assert_eq!(arg_value(&first, "--accessToken"), "0");
        assert_eq!(arg_value(&first, "--userType"), "legacy");
        assert_eq!(arg_value(&first, "--versionType"), "release");
        // The offline UUID is a stable v3 over the OfflinePlayer name.
        let uuid1 = arg_value(&first, "--uuid");
        let uuid2 = arg_value(&second, "--uuid");
        assert_eq!(uuid1, uuid2);
        let expected =
            uuid::Uuid::new_v3(&uuid::Uuid::NAMESPACE_DNS, b"OfflinePlayer:Alex").to_string();
        assert_eq!(uuid1, expected);

        // Offline mode never auto-connects: no server/port/fullscreen flags.
        for flag in [
            "--server",
            "--port",
            "--quickPlayMultiplayer",
            "--fullscreen",
        ] {
            assert!(
                !first.iter().any(|a| a == flag),
                "unexpected {flag} in {first:?}"
            );
        }
        // Default JVM memory arg is present.
        assert!(first.iter().any(|a| a == "-Xmx4G"));
    }

    #[test]
    fn offline_defaults_username_and_splits_custom_java_args() {
        let data = launch_data();
        let game_dir = Path::new("/game");

        // Blank username falls back to "Player" and blank java args to -Xmx4G.
        let command = MinecraftRunner::build_launch_command(
            &data,
            None,
            Some(""),
            None,
            game_dir,
            None,
            None,
        )
        .unwrap();
        assert_eq!(arg_value(&command, "--username"), "Player");
        assert!(command.iter().any(|a| a == "-Xmx4G"));

        // Custom JVM args are trimmed and split on whitespace.
        let command = MinecraftRunner::build_launch_command(
            &data,
            None,
            Some(" Alex "),
            Some("-Xms2G   -Xmx8G"),
            game_dir,
            None,
            None,
        )
        .unwrap();
        assert_eq!(arg_value(&command, "--username"), "Alex");
        assert!(command.iter().any(|a| a == "-Xms2G"));
        assert!(command.iter().any(|a| a == "-Xmx8G"));
        assert!(!command.iter().any(|a| a == "-Xmx4G"));
    }

    #[test]
    fn offline_profile_game_args_use_offline_tokens() {
        let mut data = launch_data();
        data.game_args = vec![
            "--username".to_string(),
            "${auth_player_name}".to_string(),
            "--accessToken".to_string(),
            "${auth_access_token}".to_string(),
            "--userType".to_string(),
            "${user_type}".to_string(),
            "--quickPlayMultiplayer".to_string(),
            "${quickPlayMultiplayer}".to_string(),
        ];
        let command = MinecraftRunner::build_launch_command(
            &data,
            None,
            Some("Alex"),
            None,
            Path::new("/game"),
            None,
            None,
        )
        .unwrap();

        assert_eq!(arg_value(&command, "--username"), "Alex");
        assert_eq!(arg_value(&command, "--accessToken"), "0");
        assert_eq!(arg_value(&command, "--userType"), "legacy");
        // The substituted quick-play pair is dropped entirely.
        assert!(!command.iter().any(|a| a == "--quickPlayMultiplayer"));
    }

    #[test]
    fn invalid_server_address_is_rejected_fail_closed() {
        let data = launch_data();
        let game_dir = Path::new("/game");
        let session = session("msa", "tok123");

        // Legitimate hosts parse fine: domains, IPv4, bracketed IPv6.
        for host in ["mc.example.com", "localhost", "127.0.0.1", "[::1]"] {
            let command = MinecraftRunner::build_launch_command(
                &data,
                Some(&session),
                None,
                None,
                game_dir,
                Some(host),
                Some(25565),
            )
            .unwrap_or_else(|e| panic!("host {host} must be accepted: {e}"));
            assert_eq!(arg_value(&command, "--server"), host);
        }

        // Garbage that could inject args or malformed URLs is refused.
        for host in [
            "not a host",
            "--username",
            "http://evil.example.com",
            "a b c",
        ] {
            let result = MinecraftRunner::build_launch_command(
                &data,
                Some(&session),
                None,
                None,
                game_dir,
                Some(host),
                Some(25565),
            );
            assert!(
                matches!(result, Err(LauncherError::InvalidInput(_))),
                "host {host:?} must be rejected, got {result:?}"
            );
        }

        // Empty-but-present host is treated as "no host" (online fallback).
        assert!(MinecraftRunner::build_launch_command(
            &data,
            Some(&session),
            None,
            None,
            game_dir,
            Some(""),
            Some(25565),
        )
        .is_ok());
    }

    /// Builds a structurally valid mod JAR (fabric metadata) at `path`.
    fn make_valid_jar(path: &Path) {
        let f = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("fabric.mod.json", options).unwrap();
        std::io::Write::write_all(&mut zip, b"{\"id\": \"ok\"}").unwrap();
        zip.start_file("com/example/Mod.class", options).unwrap();
        std::io::Write::write_all(&mut zip, b"class bytes").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn validate_mods_dir_rejects_bad_jars_before_launch() {
        let dir = std::env::temp_dir().join(format!(
            "zircon-runner-mods-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(dir.join("mods")).unwrap();

        // A clean mods/ folder passes.
        assert!(validate_mods_dir(&dir).is_ok());

        // A structurally valid mod passes.
        let good = dir.join("mods").join("good.jar");
        make_valid_jar(&good);
        assert!(validate_mods_dir(&dir).is_ok());

        // A jar without mod metadata is refused.
        let bad = dir.join("mods").join("bad.jar");
        let f = std::fs::File::create(&bad).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("META-INF/MANIFEST.MF", options).unwrap();
        std::io::Write::write_all(&mut zip, b"Manifest-Version: 1.0\n").unwrap();
        zip.finish().unwrap();
        let err = validate_mods_dir(&dir).unwrap_err();
        assert!(matches!(err, LauncherError::InvalidInput(_)));
        assert!(err.to_string().contains("bad.jar"));

        // Non-jar files are ignored.
        std::fs::write(dir.join("mods").join("readme.txt"), b"hi").unwrap();
        std::fs::remove_file(&bad).unwrap();
        assert!(validate_mods_dir(&dir).is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
