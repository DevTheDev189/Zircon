package com.mcmanager.server.web.controller;

import com.mcmanager.server.service.PackManagementService;
import io.javalin.http.Context;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.function.Supplier;

/**
 * Serves shaderpack/resourcepack downloads to the client's sync engine, mirroring
 * {@link ModController#downloadMod(Context)}. Resolved per request via
 * {@link Supplier} so these transparently follow the active instance in
 * multi-instance mode (see {@code ModServiceResolver}).
 */
public class PackFileController {

    private final Supplier<PackManagementService> packs;

    public PackFileController(Supplier<PackManagementService> packs) {
        this.packs = packs;
    }

    /** GET /files/shaderpacks/{filename} */
    public void downloadShaderpack(Context ctx) {
        stream(ctx, packs.get().getShaderpackFile(ctx.pathParam("filename")));
    }

    /** GET /files/resourcepacks/{filename} */
    public void downloadResourcepack(Context ctx) {
        stream(ctx, packs.get().getResourcepackFile(ctx.pathParam("filename")));
    }

    private void stream(Context ctx, Path file) {
        if (file == null) {
            ctx.status(404).result("Pack not found");
            return;
        }
        try {
            ctx.contentType("application/zip");
            ctx.header("Content-Disposition", "attachment; filename=\"" + file.getFileName() + "\"");
            ctx.header("Content-Length", String.valueOf(Files.size(file)));
            ctx.result(Files.newInputStream(file));
        } catch (IOException e) {
            ctx.status(500).result("Could not stream file: " + e.getMessage());
        }
    }
}
