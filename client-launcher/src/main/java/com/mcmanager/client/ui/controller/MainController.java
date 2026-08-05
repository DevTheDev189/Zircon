package com.mcmanager.client.ui.controller;

import com.mcmanager.client.auth.MicrosoftAuthService;
import com.mcmanager.client.auth.SessionData;
import com.mcmanager.client.launch.MinecraftClasspathBuilder;
import com.mcmanager.client.launch.MinecraftRunner;
import com.mcmanager.client.sync.ModSyncEngine;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.ModLoaderInfo;
import javafx.application.Platform;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.control.TextField;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * MVC controller behind {@code MainApp}: drives the sign-in → sync → launch
 * state machine and reports progress to the UI on the JavaFX thread.
 */
public class MainController {

    private static final Logger log = LoggerFactory.getLogger(MainController.class);

    private static final Path GAME_DIR = Path.of(
            System.getProperty("user.home"), ".mcmanager", "game");
    private static final String DEFAULT_SERVER_PORT = "25565";

    private final TextField serverField;
    private final Label statusLabel;
    private final ProgressBar progressBar;
    private final Button actionButton;
    private final Label userLabel;
    private final Button logoutButton;

    private final MicrosoftAuthService auth = new MicrosoftAuthService();
    private final ModSyncEngine syncEngine = new ModSyncEngine();
    private final MinecraftClasspathBuilder classpathBuilder = new MinecraftClasspathBuilder();
    private final MinecraftRunner runner = new MinecraftRunner();

    private final AtomicBoolean busy = new AtomicBoolean(false);
    private volatile SessionData session;
    private volatile Process gameProcess;

    public MainController(TextField serverField, Label statusLabel, ProgressBar progressBar,
                          Button actionButton, Label userLabel, Button logoutButton) {
        this.serverField = serverField;
        this.statusLabel = statusLabel;
        this.progressBar = progressBar;
        this.actionButton = actionButton;
        this.userLabel = userLabel;
        this.logoutButton = logoutButton;
    }

    public void init() {
        session = auth.loadCached();
        if (session != null) {
            userLabel.setText(session.getUsername());
            logoutButton.setVisible(true);
        }
        String prefill = System.getProperty("mcmanager.serverAddress");
        if (prefill != null && !prefill.isBlank()) {
            serverField.setText(prefill);
        }
        actionButton.setOnAction(e -> onAction());
        logoutButton.setOnAction(e -> onLogout());
        refreshButtonState();
    }

    public void shutdown() {
        if (gameProcess != null && gameProcess.isAlive()) {
            gameProcess.destroy();
        }
    }

    // ------------------------------------------------------------------
    // Main state machine
    // ------------------------------------------------------------------

    private void onAction() {
        if (gameProcess != null && gameProcess.isAlive()) {
            gameProcess.destroy();
            status("Game process stopped.");
            gameProcess = null;
            refreshButtonState();
            return;
        }
        if (busy.compareAndSet(false, true)) {
            String address = serverField.getText(); // captured on the FX thread
            setBusyUi(true);
            Thread.ofVirtual().name("launcher-flow").start(() -> runFlow(address));
        }
    }

    private void runFlow(String serverAddress) {
        try {
            // 1. Authenticate (sign in / silent refresh)
            if (session == null) {
                status("Opening browser for Microsoft login...");
                session = auth.login();
            } else if (session.isExpired()) {
                status("Renewing session...");
                try {
                    session = auth.refresh(session);
                } catch (Exception e) {
                    log.info("Silent refresh failed, re-authenticating: {}", e.getMessage());
                    session = auth.login();
                }
            }

            // 2. Parse the server address
            String[] hostPort = parseServerAddress(serverAddress);
            String host = hostPort[0];
            int port = Integer.parseInt(hostPort[1]);
            String baseUrl = "http://" + host + ":" + port;
            status("Server: " + baseUrl);

            // 3. Fetch the BOM so we know the MC version + loader before resolving
            BillOfMaterials bom = fetchBom(baseUrl);
            ModLoaderInfo loader = bom.getModLoader();

            // 4. Resolve the launch environment (downloads client, libs, assets)
            status("Resolving Minecraft " + bom.getMinecraftVersion() + " runtime...");
            MinecraftClasspathBuilder.LaunchData launchData =
                    classpathBuilder.resolve(bom.getMinecraftVersion(), loader, 21);

            // 5. Sync mods
            Files.createDirectories(GAME_DIR);
            status("Checking mod hashes...");
            Platform.runLater(() -> progressBar.setVisible(true));
            boolean strict = Boolean.parseBoolean(System.getProperty("mcmanager.strictVerification", "true"));
            boolean trustDirect = Boolean.parseBoolean(System.getProperty("mcmanager.trustDirectMods", "false"));
            ModSyncEngine.SyncResult syncResult = syncEngine.sync(baseUrl, GAME_DIR, strict, trustDirect,
                    new ModSyncEngine.ProgressListener() {
                        @Override
                        public void onStatus(String message) {
                            status(message);
                        }

                        @Override
                        public void onProgress(double fraction, String detail) {
                            progress(fraction);
                        }
                    });
            if (syncResult.aborted) {
                status("Sync aborted: " + syncResult.abortReason);
                return;
            }

            // 6. Launch the game, auto-connecting to the server
            status("Starting the game...");
            gameProcess = runner.launch(launchData, session, GAME_DIR, host, port, null);
            status("Game running — connecting to " + host + ":" + port);
            Thread.ofVirtual().name("game-wait").start(() -> {
                try {
                    int code = gameProcess.waitFor();
                    gameProcess = null;
                    Platform.runLater(() -> status("Game exited (code " + code + ")."));
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                } finally {
                    Platform.runLater(this::refreshButtonState);
                }
            });
        } catch (Exception e) {
            log.error("Launcher flow failed", e);
            status("Error: " + e.getMessage());
        } finally {
            Platform.runLater(() -> {
                busy.set(false);
                setBusyUi(false);
                refreshButtonState();
            });
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    private BillOfMaterials fetchBom(String baseUrl) throws IOException, InterruptedException {
        java.net.http.HttpClient client = java.net.http.HttpClient.newBuilder()
                .connectTimeout(java.time.Duration.ofSeconds(10))
                .followRedirects(java.net.http.HttpClient.Redirect.NORMAL)
                .build();
        java.net.http.HttpRequest request = java.net.http.HttpRequest.newBuilder()
                .uri(java.net.URI.create(baseUrl + "/bom"))
                .GET()
                .build();
        java.net.http.HttpResponse<String> response = client.send(request,
                java.net.http.HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("GET /bom failed: HTTP " + response.statusCode());
        }
        return com.mcmanager.core.model.BomJson.fromJson(response.body());
    }

    private String[] parseServerAddress(String input) {
        String address = input == null ? "" : input.trim();
        if (address.isEmpty()) {
            return new String[]{"localhost", DEFAULT_SERVER_PORT};
        }
        // handle "host:port" and "host"
        String host = address;
        String port = DEFAULT_SERVER_PORT;
        if (address.startsWith("[")) {
            // IPv6 literal [::1]:25565
            int end = address.indexOf(']');
            if (end > 0) {
                host = address.substring(1, end);
                if (end + 1 < address.length() && address.charAt(end + 1) == ':') {
                    port = address.substring(end + 2);
                }
            }
        } else {
            int colon = address.lastIndexOf(':');
            if (colon > 0) {
                host = address.substring(0, colon);
                port = address.substring(colon + 1);
            }
        }
        return new String[]{host, port};
    }

    private void onLogout() {
        try {
            auth.clearCache();
        } catch (IOException e) {
            log.warn("Could not clear auth cache", e);
        }
        session = null;
        userLabel.setText("Not signed in");
        logoutButton.setVisible(false);
        status("Signed out.");
        refreshButtonState();
    }

    // ------------------------------------------------------------------
    // UI updates (must run on the JavaFX thread)
    // ------------------------------------------------------------------

    private void status(String text) {
        Platform.runLater(() -> statusLabel.setText(text));
    }

    private void progress(double fraction) {
        Platform.runLater(() -> {
            progressBar.setProgress(fraction);
            progressBar.setVisible(true);
        });
    }

    private void setBusyUi(boolean busy) {
        Platform.runLater(() -> {
            progressBar.setProgress(busy ? ProgressBar.INDETERMINATE_PROGRESS : 0);
            progressBar.setVisible(busy);
        });
    }

    private void refreshButtonState() {
        boolean gameAlive = gameProcess != null && gameProcess.isAlive();
        String greenStyle = "-fx-background-color: #2da44e; -fx-text-fill: white;"
                + "-fx-font-size: 18px; -fx-font-weight: bold; -fx-background-radius: 10;";
        if (gameAlive) {
            actionButton.setText("STOP GAME");
            actionButton.setStyle("-fx-background-color: #cf222e; -fx-text-fill: white;"
                    + "-fx-font-size: 18px; -fx-font-weight: bold; -fx-background-radius: 10;");
        } else if (busy.get()) {
            actionButton.setText("WORKING...");
            actionButton.setDisable(true);
        } else if (session == null) {
            actionButton.setText("SIGN IN WITH MICROSOFT");
            actionButton.setStyle(greenStyle);
        } else {
            actionButton.setText("PLAY");
            actionButton.setStyle(greenStyle);
        }
    }
}
