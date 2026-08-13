package com.mcmanager.server.web.controller;

import com.mcmanager.core.model.BomJson;
import com.mcmanager.server.service.BomService;
import com.mcmanager.server.service.ModServiceResolver;
import io.javalin.http.Context;

/**
 * Publishes the {@link com.mcmanager.core.model.BillOfMaterials} that the client
 * launcher syncs against.
 *
 * <p>In multi-instance mode the BOM is resolved from the {@code Host} header port
 * of the request: a client connecting to an instance's dedicated port (e.g.
 * {@code localhost:25566}) receives exactly that instance's BOM, and the shared
 * main port (25565) falls back to the active instance. This keeps the version and
 * mod list the launcher resolves in lockstep with the game server it then joins.
 */
public class BomController {

    private final ModServiceResolver resolver;

    public BomController(ModServiceResolver resolver) {
        this.resolver = resolver;
    }

    /** GET /bom — full BOM as JSON for the instance owning the request's port. */
    public void getBom(Context ctx) {
        ctx.contentType("application/json; charset=utf-8");
        ctx.result(BomJson.toJson(resolveBom(ctx).getBom()));
    }

    private BomService resolveBom(Context ctx) {
        Integer port = ModServiceResolver.hostPort(ctx);
        if (port != null) {
            BomService perInstance = resolver.bomByExternalPort(port);
            if (perInstance != null) {
                return perInstance;
            }
        }
        return resolver.bom(); // active instance, or the legacy single-server store
    }
}
