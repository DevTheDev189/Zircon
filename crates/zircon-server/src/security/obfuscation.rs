//! Compile-time obfuscation helpers for embedded secrets.

include!(concat!(env!("OUT_DIR"), "/curseforge_key_obf.rs"));

/// Returns the embedded CurseForge API key deobfuscated in memory on demand.
pub fn embedded_curseforge_key() -> String {
    deobfuscate_embedded_key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_key_deobfuscates_correctly() {
        let key = embedded_curseforge_key();
        assert!(!key.is_empty(), "Embedded key must not be empty");
        assert!(
            key.starts_with("$2a$10$"),
            "Key format must match expected CurseForge token format"
        );
    }
}
