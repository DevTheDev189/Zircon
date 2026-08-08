package com.mcmanager.server.service;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.instance.ServerInstanceManager;

import java.nio.file.Path;

/**
 * Resolves the BOM / mod services behind the client-facing legacy endpoints
 * ({@code /bom}, {@code /files/mods/*}, {@code /api/mods/*}).
 *
 * <p>When the wrapper manages instances, these endpoints serve the ACTIVE
 * instance's data — freshly constructed from disk on every call (mirroring
 * {@code InstanceController}), so the client always syncs against the same
 * mods the admin UI manages and the two stores can never drift. When no
 * instances exist, the legacy single-server store is served for backwards
 * compatibility.
 */
public class ModServiceResolver {

    private final ServerInstanceManager instanceManager;
    private final BomService legacyBom;
    private final ModManagementService legacyMods;
    private final String curseForgeApiKey;

    public ModServiceResolver(ServerInstanceManager instanceManager, BomService legacyBom,
                              ModManagementService legacyMods, String curseForgeApiKey) {
        this.instanceManager = instanceManager;
        this.legacyBom = legacyBom;
        this.legacyMods = legacyMods;
        this.curseForgeApiKey = curseForgeApiKey == null ? "" : curseForgeApiKey;
    }

    /** @return the active instance, or {@code null} in pure legacy mode. */
    public InstanceConfig activeInstance() {
        return instanceManager.getActiveInstance();
    }

    /** Resolves the BOM service backing {@code GET /bom}. */
    public BomService bom() {
        InstanceConfig active = activeInstance();
        return active == null ? legacyBom : instanceService(active).bom;
    }

    /** Resolves the mod service backing {@code /files/mods/*} and {@code /api/mods/*}. */
    public ModManagementService mods() {
        InstanceConfig active = activeInstance();
        return active == null ? legacyMods : instanceService(active).mods;
    }

    /** Freshly built per-instance service pair (disk is always the source of truth). */
    private InstanceServices instanceService(InstanceConfig cfg) {
        Path instanceDir = instanceManager.getInstanceDir(cfg.getId());
        BomService bom = new BomService(instanceDir.resolve("bom.json"),
                new BillOfMaterials(cfg.getMinecraftVersion(), cfg.getModLoader(), cfg.getName()));
        ModManagementService mods = new ModManagementService(bom, instanceDir.resolve("mods"),
                curseForgeApiKey);
        return new InstanceServices(bom, mods);
    }

    private record InstanceServices(BomService bom, ModManagementService mods) {
    }
}
