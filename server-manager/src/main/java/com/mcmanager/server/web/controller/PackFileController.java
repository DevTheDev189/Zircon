package com.mcmanager.server.web.controller;

import com.mcmanager.server.service.ModServiceResolver;
import com.mcmanager.server.service.PackManagementService;
import io.javalin.http.Context;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Serves shaderpack/resourcepack downloads to the client's sync engine, mirroring
 * {@link ModController#downloadMod(Context)}. Resolved per request from the
 * {@code Host} header port so downloads follow the instance whose port the client
 * connected through (falling back to the active instance).
 */
public class PackFileController {

    private final ModServiceResolver resolver;

    public PackFileController(ModServiceResolver resolver) {
        this.resolver = resolver;
    }

    /** The pack service for the instance owning the request's port, else the active instance. */
    private PackManagementService resolvePacks(Context ctx) {
        Integer port = ModServiceResolver.hostPort(ctx);
        if (port != null) {
            PackManagementService perInstance = resolver.packsByExternalPort(port);
            if (perInstance != null) {
                return perInstance;
            }
        }
        return resolver.packs();
    }

    /** GET /files/shaderpacks/{filename} */
    public void downloadShaderpack(Context ctx) {
        stream(ctx, resolvePacks(ctx).getShaderpackFile(ctx.pathParam("filename")));
    }

    /** GET /files/resourcepacks/{filename} */
    public void downloadResourcepack(Context ctx) {
        stream(ctx, resolvePacks(ctx).getResourcepackFile(ctx.pathParam("filename")));
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
