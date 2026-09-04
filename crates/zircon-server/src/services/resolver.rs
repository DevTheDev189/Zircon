//! Resolves the BOM / mod services behind the client-facing legacy endpoints
//! (`/bom`, `/files/mods/*`, `/api/mods/*`).
//!
//! When the wrapper manages instances, these endpoints serve the ACTIVE
//! instance's data — freshly constructed from disk on every call, so the client
//! always syncs against the same mods the admin UI manages and the two stores
//! can never drift. When no instances exist, the legacy single-server store is
//! served for backwards compatibility.
//!
//! Port of `com.mcmanager.server.service.ModServiceResolver`.

use std::sync::Arc;

use ed25519_dalek::SigningKey;
use zircon_core::model::{BillOfMaterials, InstanceConfig};

use super::bom::BomService;
use super::mods::ModManagementService;
use super::packs::PackManagementService;
use crate::instance::ServerInstanceManager;

/// Freshly built per-instance service trio (disk is always the source of truth).
#[derive(Clone)]
pub struct InstanceServices {
    pub bom: Arc<BomService>,
    pub mods: ModManagementService,
    pub packs: PackManagementService,
}

/// Resolves the services backing the client-facing legacy endpoints.
pub struct ModServiceResolver {
    instance_manager: Arc<ServerInstanceManager>,
    legacy_bom: Arc<BomService>,
    legacy_mods: Arc<ModManagementService>,
    legacy_packs: PackManagementService,
    curse_forge_api_key: String,
    /// Server-level Ed25519 key: instance BOMs are signed with the same key as
    /// the legacy store so launchers verify every BOM with one pin.
    signing_key: Option<Arc<SigningKey>>,
}

impl ModServiceResolver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instance_manager: Arc<ServerInstanceManager>,
        legacy_bom: Arc<BomService>,
        legacy_mods: Arc<ModManagementService>,
        legacy_packs: PackManagementService,
        curse_forge_api_key: &str,
        signing_key: Option<Arc<SigningKey>>,
    ) -> Self {
        Self {
            instance_manager,
            legacy_bom,
            legacy_mods,
            legacy_packs,
            curse_forge_api_key: curse_forge_api_key.to_string(),
            signing_key,
        }
    }

    /// The active instance, or `None` in pure legacy mode.
    pub fn active_instance(&self) -> Option<InstanceConfig> {
        self.instance_manager.get_active_instance()
    }

    /// The instance owning `external_port`, or `None`.
    pub fn instance_by_external_port(&self, external_port: i32) -> Option<InstanceConfig> {
        self.instance_manager.find_by_external_port(external_port)
    }

    /// BOM service of the instance owning the port, or `None` when unowned.
    pub fn bom_by_external_port(&self, external_port: i32) -> Option<Arc<BomService>> {
        self.instance_by_external_port(external_port)
            .map(|cfg| self.instance_service(&cfg).bom)
    }

    /// Mod service of the instance owning the port, or `None` when unowned.
    pub fn mods_by_external_port(&self, external_port: i32) -> Option<ModManagementService> {
        self.instance_by_external_port(external_port)
            .map(|cfg| self.instance_service(&cfg).mods)
    }

    /// Pack service of the instance owning the port, or `None` when unowned.
    pub fn packs_by_external_port(&self, external_port: i32) -> Option<PackManagementService> {
        self.instance_by_external_port(external_port)
            .map(|cfg| self.instance_service(&cfg).packs)
    }

    /// The port from a request's `Host` header (e.g. `localhost:25566` →
    /// `25566`), or `None` when absent or unparseable.
    pub fn host_port(host: Option<&str>) -> Option<i32> {
        let host = host?.trim();
        if host.is_empty() {
            return None;
        }
        let port_str = if host.starts_with('[') {
            // IPv6 literal, e.g. [::1]:25565
            let end = host.find(']')?;
            if end + 1 >= host.len() || host.as_bytes()[end + 1] != b':' {
                return None;
            }
            &host[end + 2..]
        } else {
            host.rfind(':')?;
            &host[host.rfind(':')? + 1..]
        };
        port_str.parse::<i32>().ok()
    }

    /// Resolves the BOM service backing `GET /bom`.
    pub fn bom(&self) -> Arc<BomService> {
        match self.active_instance() {
            Some(active) => self.instance_service(&active).bom,
            None => self.legacy_bom.clone(),
        }
    }

    /// Resolves the mod service backing `/files/mods/*` and `/api/mods/*`.
    pub fn mods(&self) -> ModManagementService {
        match self.active_instance() {
            Some(active) => self.instance_service(&active).mods,
            None => self.legacy_mods.as_ref().clone(),
        }
    }

    /// Resolves the pack service backing `/files/shaderpacks/*` etc.
    pub fn packs(&self) -> PackManagementService {
        match self.active_instance() {
            Some(active) => self.instance_service(&active).packs,
            None => self.legacy_packs.clone(),
        }
    }

    /// Freshly built per-instance service trio (disk is always the source of truth).
    pub fn instance_service(&self, cfg: &InstanceConfig) -> InstanceServices {
        let instance_dir = self.instance_manager.get_instance_dir(&cfg.id);
        let bom = Arc::new(
            BomService::new(
                instance_dir.join("bom.json"),
                Some(BillOfMaterials::new(
                    cfg.minecraft_version.clone(),
                    cfg.mod_loader.clone(),
                    Some(cfg.name.clone()),
                )),
            )
            .with_signing_key(self.signing_key.clone()),
        );
        let mods = ModManagementService::new(
            bom.clone(),
            instance_dir.join("mods"),
            &self.curse_forge_api_key,
        );
        let packs = PackManagementService::new(
            bom.clone(),
            instance_dir.join("shaderpacks"),
            instance_dir.join("resourcepacks"),
        );
        InstanceServices { bom, mods, packs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_port_parsing() {
        assert_eq!(
            Some(25566),
            ModServiceResolver::host_port(Some("localhost:25566"))
        );
        assert_eq!(
            Some(25565),
            ModServiceResolver::host_port(Some("127.0.0.1:25565"))
        );
        assert_eq!(
            Some(25565),
            ModServiceResolver::host_port(Some("[::1]:25565"))
        );
        assert_eq!(None, ModServiceResolver::host_port(Some("localhost")));
        assert_eq!(None, ModServiceResolver::host_port(Some("")));
        assert_eq!(
            None,
            ModServiceResolver::host_port(Some("localhost:notaport"))
        );
        assert_eq!(None, ModServiceResolver::host_port(None));
    }
}
