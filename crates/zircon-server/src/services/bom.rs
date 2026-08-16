//! Loads, caches and persists the `BillOfMaterials`. The BOM is the single
//! source of truth the client launcher syncs against, so every mutation goes
//! through this type and is written to `bom.json` immediately.
//!
//! Port of `com.mcmanager.server.service.BomService`.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use zircon_core::model::BillOfMaterials;

/// Loads, caches and persists the `BillOfMaterials`.
pub struct BomService {
    bom_file: PathBuf,
    default_bom: Option<BillOfMaterials>,
    bom: Mutex<Option<BillOfMaterials>>,
}

impl BomService {
    /// Instance-scoped BOM stored at `<instance>/bom.json`.
    pub fn new(bom_file: PathBuf, default_bom: Option<BillOfMaterials>) -> Self {
        Self {
            bom_file,
            default_bom,
            bom: Mutex::new(None),
        }
    }

    /// Returns a clone of the current BOM, loading it (or creating a default)
    /// on first access.
    pub fn get_bom(&self) -> BillOfMaterials {
        let mut guard = self.bom.lock().unwrap();
        if guard.is_none() {
            *guard = Some(self.load());
        }
        guard.as_ref().unwrap().clone()
    }

    /// Mutates the current BOM in place (loading it first).
    pub fn with_bom<R>(&self, f: impl FnOnce(&mut BillOfMaterials) -> R) -> R {
        let mut guard = self.bom.lock().unwrap();
        if guard.is_none() {
            *guard = Some(self.load());
        }
        f(guard.as_mut().unwrap())
    }

    /// Persists the current BOM to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let bom = self.get_bom();
        self.save_bom(&bom)
    }

    pub fn has_bom_file(&self) -> bool {
        self.bom_file.is_file()
    }

    fn load(&self) -> BillOfMaterials {
        if self.bom_file.is_file() {
            match fs::read_to_string(&self.bom_file)
                .map_err(|e| std::io::Error::other(e.to_string()))
                .and_then(|content| {
                    serde_json::from_str::<BillOfMaterials>(&content)
                        .map_err(|e| std::io::Error::other(e.to_string()))
                }) {
                Ok(parsed) => {
                    tracing::info!(
                        "Loaded BOM: {} mods for MC {}",
                        parsed.mods.len(),
                        parsed.minecraft_version
                    );
                    return parsed;
                }
                Err(e) => {
                    tracing::warn!(
                        "Could not parse {}, recreating: {e}",
                        self.bom_file.display()
                    );
                }
            }
        }
        match &self.default_bom {
            Some(default) => {
                if let Err(e) = self.save_bom(default) {
                    tracing::warn!("Could not write default BOM: {e}");
                }
                default.clone()
            }
            None => {
                tracing::warn!("No default BOM configured for {}", self.bom_file.display());
                BillOfMaterials::default()
            }
        }
    }

    fn save_bom(&self, bom: &BillOfMaterials) -> std::io::Result<()> {
        let json =
            serde_json::to_string_pretty(bom).map_err(|e| std::io::Error::other(e.to_string()))?;
        fs::write(&self.bom_file, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("bom")
    }

    #[test]
    fn default_bom_is_persisted_on_first_access() {
        let dir = temp_dir();
        let file = dir.join("bom.json");
        let default = BillOfMaterials::new("1.20.4", None, Some("Server".to_string()));
        let service = BomService::new(file.clone(), Some(default));

        let bom = service.get_bom();
        assert_eq!("1.20.4", bom.minecraft_version);
        assert!(file.is_file());

        service.with_bom(|b| {
            b.mods.push(zircon_core::model::ModEntry::new(
                Some("m".to_string()),
                "m.jar",
                None,
                0,
                None,
                None,
                1,
            ));
        });
        service.save().unwrap();

        let reloaded = BomService::new(file.clone(), None).get_bom();
        assert_eq!(1, reloaded.mods.len());
        let _ = fs::remove_dir_all(&dir);
    }
}
