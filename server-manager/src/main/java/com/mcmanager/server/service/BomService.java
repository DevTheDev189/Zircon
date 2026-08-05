package com.mcmanager.server.service;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.BomJson;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Loads, caches and persists the {@link BillOfMaterials}. The BOM is the single
 * source of truth the client launcher syncs against, so every mutation goes
 * through this class and is written to {@code bom.json} immediately.
 */
public class BomService {

    private static final Logger log = LoggerFactory.getLogger(BomService.class);

    private final Path bomFile;
    private final ConfigService configService;
    private BillOfMaterials bom;

    public BomService(ConfigService configService) {
        this.configService = configService;
        this.bomFile = configService.getBomFile();
    }

    /** Returns the current BOM, loading it (or creating a default) on first access. */
    public synchronized BillOfMaterials getBom() {
        if (bom == null) {
            bom = load();
        }
        return bom;
    }

    /** Persists the current BOM to disk. */
    public synchronized void save() throws IOException {
        Files.writeString(bomFile, BomJson.toJson(getBom()), StandardCharsets.UTF_8);
    }

    public synchronized boolean hasBomFile() {
        return Files.exists(bomFile);
    }

    private BillOfMaterials load() {
        if (Files.exists(bomFile)) {
            try {
                BillOfMaterials parsed = BomJson.fromJson(Files.readString(bomFile));
                if (parsed != null) {
                    log.info("Loaded BOM: {} mods for MC {}", parsed.getMods().size(),
                            parsed.getMinecraftVersion());
                    return parsed;
                }
            } catch (IOException | RuntimeException e) {
                log.warn("Could not parse {}, recreating", bomFile, e);
            }
        }
        return createDefault();
    }

    private BillOfMaterials createDefault() {
        ConfigService.ServerConfig cfg = configService.getConfig();
        BillOfMaterials fresh = new BillOfMaterials(
                cfg.minecraftVersion,
                new com.mcmanager.core.model.ModLoaderInfo(
                        cfg.modLoader.getType(),
                        cfg.modLoader.getVersion(),
                        cfg.modLoader.getLoaderJarUrl()),
                cfg.serverTitle);
        try {
            save(fresh);
        } catch (IOException e) {
            log.warn("Could not write default BOM", e);
        }
        return fresh;
    }

    private void save(BillOfMaterials bom) throws IOException {
        Files.writeString(bomFile, BomJson.toJson(bom), StandardCharsets.UTF_8);
    }
}
