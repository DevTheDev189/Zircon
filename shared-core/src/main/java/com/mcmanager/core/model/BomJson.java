package com.mcmanager.core.model;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonSyntaxException;

/**
 * Thin Gson wrapper for serializing / deserializing {@link BillOfMaterials}.
 * Centralizes the Gson instance so all modules agree on formatting.
 */
public final class BomJson {

    private static final Gson GSON = new GsonBuilder()
            .setPrettyPrinting()
            .disableHtmlEscaping()
            .create();

    private BomJson() {
    }

    public static String toJson(BillOfMaterials bom) {
        return GSON.toJson(bom);
    }

    public static BillOfMaterials fromJson(String json) throws JsonSyntaxException {
        return GSON.fromJson(json, BillOfMaterials.class);
    }

    public static Gson gson() {
        return GSON;
    }
}
