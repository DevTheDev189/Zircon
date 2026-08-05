package com.mcmanager.server.web.controller;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.BomJson;
import com.mcmanager.server.service.BomService;
import io.javalin.http.Context;

/**
 * Publishes the {@link BillOfMaterials} that the client launcher syncs against.
 */
public class BomController {

    private final BomService bomService;

    public BomController(BomService bomService) {
        this.bomService = bomService;
    }

    /** GET /bom — full BOM as JSON. */
    public void getBom(Context ctx) {
        ctx.contentType("application/json; charset=utf-8");
        ctx.result(BomJson.toJson(bomService.getBom()));
    }
}
