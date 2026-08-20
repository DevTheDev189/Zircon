//! Filesystem-traversal security integration tests (Phase 5 matrix).

use std::path::PathBuf;

use zircon_launcher_lib::error::LauncherError;
use zircon_launcher_lib::offline::OfflineInstanceManager;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zircon-traversal-sec-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Attempting to delete `../../foo.txt` (or any non-basename) via the offline
/// instance mods API must return `InvalidInput` — a malicious caller cannot
/// use the launcher to delete files outside the instance's `mods/` directory.
#[test]
fn delete_mod_traversal_blocked() {
    let base = temp_dir();
    let manager = OfflineInstanceManager::new(base.clone());
    let instance = manager
        .create("Traversal", "1.20.4", "fabric", "0.15.11")
        .expect("create instance");

    // A decoy file OUTSIDE the instance directory that a traversal payload
    // would try to reach; it must remain untouched.
    let outside = base.join("outside.txt");
    std::fs::write(&outside, b"must survive").unwrap();

    for payload in [
        "../../foo.txt",
        "..\\foo.txt",
        "mods/../../foo.txt",
        "/etc/passwd",
    ] {
        let err = manager
            .delete_mod(&instance, payload)
            .expect_err("traversal filename must be rejected");
        assert!(
            matches!(err, LauncherError::InvalidInput(_)),
            "{payload:?} must be InvalidInput, got {err:?}"
        );
    }
    assert_eq!(
        "must survive",
        std::fs::read_to_string(&outside).unwrap(),
        "files outside the instance mods dir must never be touched"
    );

    // A legitimate basename still deletes fine.
    let mods_dir = manager.instance_dir(&instance.id).join("mods");
    std::fs::create_dir_all(&mods_dir).unwrap();
    std::fs::write(mods_dir.join("ok.jar"), b"jar").unwrap();
    assert!(manager.delete_mod(&instance, "ok.jar").is_ok());
    assert!(!mods_dir.join("ok.jar").exists());

    let _ = std::fs::remove_dir_all(&base);
}
