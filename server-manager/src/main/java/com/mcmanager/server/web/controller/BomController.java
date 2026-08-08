package com.mcmanager.server.web.controller;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.BomJson;
import com.mcmanager.server.service.BomService;
import io.javalin.http.Context;

import java.util.function.Supplier;

/**
 * Publishes the {@link BillOfMaterials} that the client launcher syncs against.
 *
 * <p>The BOM source is resolved per request via {@link Supplier} so the endpoint
 * transparently serves the active instance's BOM when the wrapper runs in
 * multi-instance mode (see {@code ModServiceResolver}).
 */
public class BomController {

    private final Supplier<BomService> bomService;

    public BomController(Supplier<BomService> bomService) {
        this.bomService = bomService;
    }

    /** GET /bom — full BOM as JSON. */
    public void getBom(Context ctx) {
        ctx.contentType("application/json; charset=utf-8");
        ctx.result(BomJson.toJson(bomService.get().getBom()));
    }
}
