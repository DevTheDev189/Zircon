package com.mcmanager.server.process;

import com.mcmanager.server.service.ConfigService;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.EnabledIfEnvironmentVariable;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Full end-to-end wrapper test: installs a real NeoForge server (official
 * installer, headless) into a temp data dir and boots it through
 * {@link MinecraftProcessManager}, waiting for the "Done" line.
 *
 * <p>Heavy (downloads the installer + server libraries) — only runs when
 * requested:
 * <pre>
 * MC_WRAPPER_E2E=1 ./gradlew :server-manager:test --tests "*ServerWrapperE2ETest"
 * </pre>
 */
@EnabledIfEnvironmentVariable(named = "MC_WRAPPER_E2E", matches = "1")
class ServerWrapperE2ETest {

    @TempDir
    Path tempDir;

    @Test
    void installsAndBootsNeoForgeServer() throws Exception {
        Path dataDir = tempDir.resolve("server-data");
        Path serverDir = dataDir.resolve("server");
        Files.createDirectories(serverDir);
        // Accept EULA like an operator would after the first run.
        Files.writeString(serverDir.resolve("eula.txt"), "eula=true\n", StandardCharsets.UTF_8);

        Files.writeString(dataDir.resolve("config.json"), """
                {
                  "webPort": 25597,
                  "mcPort": 25599,
                  "publicPort": 25598,
                  "serverTitle": "E2E Test Server",
                  "minecraftVersion": "1.20.4",
                  "modLoader": {"type": "neoforge", "version": "20.4.250"},
                  "javaArgs": "-Xms1G -Xmx2G",
                  "autoStartServer": false
                }
                """, StandardCharsets.UTF_8);

        System.setProperty("mcmanager.dataDir", dataDir.toString());
        ConfigService config = new ConfigService();
        ConsoleStreamHandler console = new ConsoleStreamHandler();
        MinecraftProcessManager processManager = new MinecraftProcessManager(config, console);

        try {
            processManager.start(); // installs the server on demand, then launches

            long deadline = System.currentTimeMillis() + 5 * 60_000L;
            boolean done = false;
            while (System.currentTimeMillis() < deadline && processManager.isRunning()) {
                List<String> recent = console.recentHistory(500);
                if (recent.stream().anyMatch(line -> line.contains("Done ("))) {
                    done = true;
                    break;
                }
                Thread.sleep(3_000);
            }

            assertTrue(done, "NeoForge server did not report Done. Console tail:\n"
                    + String.join("\n", console.recentHistory(40)));
        } finally {
            processManager.stop();
        }
    }
}
