package com.mcmanager.client.sync;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.PackEntry;
import com.sun.net.httpserver.HttpServer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Verifies pack reconciliation against the BOM: server packs that are no longer
 * advertised get pruned from the client, while locally added packs are preserved.
 */
class PackSyncEngineTest {

    @TempDir
    Path gameDir;

    private HttpServer server;
    private String baseUrl;

    @BeforeEach
    void startServer() throws IOException {
        server = HttpServer.create(new InetSocketAddress(0), 0);
        server.createContext("/files/shaderpacks/", exchange -> {
            byte[] body = "shader-zip-bytes".getBytes(StandardCharsets.UTF_8);
            exchange.sendResponseHeaders(200, body.length);
            try (OutputStream os = exchange.getResponseBody()) {
                os.write(body);
            }
        });
        server.start();
        baseUrl = "http://localhost:" + server.getAddress().getPort();
    }

    @AfterEach
    void stopServer() {
        server.stop(0);
    }

    @Test
    void prunesServerPackNoLongerInBom() throws Exception {
        // A previously synced server pack sits on disk but is no longer advertised.
        Files.createDirectories(gameDir.resolve("shaderpacks"));
        Files.writeString(gameDir.resolve("shaderpacks/OldShader.zip"), "old");

        BillOfMaterials bom = new BillOfMaterials("1.21.4", null, "t");
        bom.addShaderpack(new PackEntry("new-pack", "NewShader.zip", null, 0,
                "modrinth", baseUrl + "/files/shaderpacks/NewShader.zip", 1));

        new PackSyncEngine().sync(bom, baseUrl, gameDir, Set.of(), Set.of(), msg -> {
        });

        assertFalse(Files.exists(gameDir.resolve("shaderpacks/OldShader.zip")),
                "old server pack must be pruned when absent from the BOM");
        assertTrue(Files.exists(gameDir.resolve("shaderpacks/NewShader.zip")),
                "new server pack must be downloaded");
    }

    @Test
    void keepsLocallyAddedPackWhenBomEmpty() throws Exception {
        // Server owner removed all packs; the player's own pack must survive.
        Files.createDirectories(gameDir.resolve("shaderpacks"));
        Files.writeString(gameDir.resolve("shaderpacks/MyCustom.zip"), "mine");

        BillOfMaterials bom = new BillOfMaterials("1.21.4", null, "t");

        new PackSyncEngine().sync(bom, baseUrl, gameDir, Set.of("MyCustom.zip"), Set.of(), msg -> {
        });

        assertTrue(Files.exists(gameDir.resolve("shaderpacks/MyCustom.zip")),
                "locally added pack must never be pruned");
    }

    @Test
    void emptyBomPrunesStaleServerPacks() throws Exception {
        Files.createDirectories(gameDir.resolve("shaderpacks"));
        Files.writeString(gameDir.resolve("shaderpacks/RemovedPack.zip"), "old");

        BillOfMaterials bom = new BillOfMaterials("1.21.4", null, "t");

        new PackSyncEngine().sync(bom, baseUrl, gameDir, Set.of(), Set.of(), msg -> {
        });

        assertFalse(Files.exists(gameDir.resolve("shaderpacks/RemovedPack.zip")),
                "packs not in the BOM must be deleted even when the BOM has no packs");
    }
}
