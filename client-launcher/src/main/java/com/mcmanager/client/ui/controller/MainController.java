package com.mcmanager.client.ui.controller;

import com.mcmanager.client.auth.MicrosoftAuthService;
import com.mcmanager.client.auth.SessionData;
import com.mcmanager.client.launch.JavaRuntimeSelector;
import com.mcmanager.client.launch.MinecraftClasspathBuilder;
import com.mcmanager.client.launch.MinecraftRunner;
import com.mcmanager.client.model.SavedServer;
import com.mcmanager.client.skin.SkinManager;
import com.mcmanager.client.sync.ModSyncEngine;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.BomJson;
import com.mcmanager.core.model.ModLoaderInfo;
import javafx.application.Platform;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Node;
import javafx.scene.control.*;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.layout.*;
import javafx.stage.FileChooser;
import javafx.stage.Stage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Controller driving navigation views, server list management,
 * custom skin uploads, settings, dynamic mod staging sync, and game launches.
 */
public class MainController {

    private static final Logger log = LoggerFactory.getLogger(MainController.class);
    private static final String DEFAULT_SERVER_PORT = "25565";

    private static final Path INSTANCES_ROOT = Path.of(
            System.getProperty("user.home"), ".zircon", "instances");

    // Sidebar & View Controls
    private final Button navServerList;
    private final Button navChangeSkin;
    private final Button navSettings;
    private final Node serverListView;
    private final Node changeSkinView;
    private final Node settingsView;

    // Server List View Controls
    private final VBox savedServersContainer;
    private final VBox recommendedContainer;
    private final Button addServerBtn;

    // Skin View Controls
    private final ImageView skinPreview;
    private final Button uploadSkinBtn;
    private final Button resetSkinBtn;
    private final Label skinStatus;

    // Settings Controls
    private final Slider ramSlider;
    private final Label ramLabel;
    private final CheckBox strictVerifyCheck;
    private final CheckBox trustDirectCheck;
    private final TextField clientIdField;

    // Global Status & Auth Controls
    private final Label statusLabel;
    private final ProgressBar progressBar;
    private final Label userLabel;
    private final Button logoutButton;
    private final Stage stage;

    private final MicrosoftAuthService auth = new MicrosoftAuthService();
    private final ModSyncEngine syncEngine = new ModSyncEngine();
    private final MinecraftClasspathBuilder classpathBuilder = new MinecraftClasspathBuilder();
    private final MinecraftRunner runner = new MinecraftRunner();

    private final AtomicBoolean busy = new AtomicBoolean(false);
    private volatile SessionData session;
    private volatile Process gameProcess;

    public MainController(Button navServerList, Button navChangeSkin, Button navSettings,
                          Node serverListView, Node changeSkinView, Node settingsView,
                          VBox savedServersContainer, VBox recommendedContainer, Button addServerBtn,
                          ImageView skinPreview, Button uploadSkinBtn, Button resetSkinBtn, Label skinStatus,
                          Slider ramSlider, Label ramLabel, CheckBox strictVerifyCheck, CheckBox trustDirectCheck,
                          TextField clientIdField, Label statusLabel, ProgressBar progressBar,
                          Label userLabel, Button logoutButton, Stage stage) {
        this.navServerList = navServerList;
        this.navChangeSkin = navChangeSkin;
        this.navSettings = navSettings;
        this.serverListView = serverListView;
        this.changeSkinView = changeSkinView;
        this.settingsView = settingsView;
        this.savedServersContainer = savedServersContainer;
        this.recommendedContainer = recommendedContainer;
        this.addServerBtn = addServerBtn;
        this.skinPreview = skinPreview;
        this.uploadSkinBtn = uploadSkinBtn;
        this.resetSkinBtn = resetSkinBtn;
        this.skinStatus = skinStatus;
        this.ramSlider = ramSlider;
        this.ramLabel = ramLabel;
        this.strictVerifyCheck = strictVerifyCheck;
        this.trustDirectCheck = trustDirectCheck;
        this.clientIdField = clientIdField;
        this.statusLabel = statusLabel;
        this.progressBar = progressBar;
        this.userLabel = userLabel;
        this.logoutButton = logoutButton;
        this.stage = stage;
    }

    public void init() {
        // Setup Navigation Tabs
        navServerList.setOnAction(e -> switchTab(serverListView, navServerList));
        navChangeSkin.setOnAction(e -> switchTab(changeSkinView, navChangeSkin));
        navSettings.setOnAction(e -> switchTab(settingsView, navSettings));
        switchTab(serverListView, navServerList);

        // Auth initialization — Microsoft sign-in is mandatory; the cached session
        // (or a fresh browser login) is loaded on startup and during launches.
        initSession();

        // Server List Setup
        addServerBtn.setOnAction(e -> promptAddServer());
        populateServerList();
        populateRecommendedServers();

        // Skin Customizer Setup
        refreshSkinPreview();
        uploadSkinBtn.setOnAction(e -> handleUploadSkin());
        resetSkinBtn.setOnAction(e -> handleResetSkin());

        // Settings Setup
        ramSlider.valueProperty().addListener((obs, oldVal, newVal) -> {
            ramLabel.setText("Max Memory Allocation (RAM): " + newVal.intValue() + " GB");
        });

        logoutButton.setOnAction(e -> onLogout());
    }

    private void switchTab(Node targetView, Button activeBtn) {
        serverListView.setVisible(targetView == serverListView);
        changeSkinView.setVisible(targetView == changeSkinView);
        settingsView.setVisible(targetView == settingsView);

        for (Button btn : new Button[]{navServerList, navChangeSkin, navSettings}) {
            if (btn == activeBtn) {
                btn.setStyle("-fx-font-size: 14px; -fx-padding: 10 14; -fx-background-radius: 8; "
                        + "-fx-background-color: #21262d; -fx-text-fill: white; -fx-font-weight: bold;");
            } else {
                btn.setStyle("-fx-font-size: 14px; -fx-padding: 10 14; -fx-background-radius: 8; "
                        + "-fx-background-color: transparent; -fx-text-fill: #c9d1d9;");
            }
        }
    }

    private void initSession() {
        session = auth.loadCached();
        if (session != null) {
            userLabel.setText(session.getUsername());
            logoutButton.setVisible(true);
            status("Signed in as " + session.getUsername());
        } else {
            userLabel.setText("Not signed in");
            logoutButton.setVisible(false);
            status("Ready to sign in.");
        }
    }

    public void shutdown() {
        if (gameProcess != null && gameProcess.isAlive()) {
            gameProcess.destroy();
        }
    }

    // ------------------------------------------------------------------
    // Server List Management
    // ------------------------------------------------------------------

    private void populateServerList() {
        savedServersContainer.getChildren().clear();
        List<SavedServer> saved = SavedServer.load();
        if (saved.isEmpty()) {
            // Seed a local default server on first run
            SavedServer.recordPlayed("Localhost Server", "localhost:25565");
            saved = SavedServer.load();
        }

        for (SavedServer s : saved) {
            savedServersContainer.getChildren().add(createSavedServerCard(s));
        }
    }

    private HBox createSavedServerCard(SavedServer server) {
        Label nameLbl = new Label(server.getName());
        nameLbl.setStyle("-fx-font-size: 14px; -fx-font-weight: bold; -fx-text-fill: white;");

        Label addrLbl = new Label(server.getAddress());
        addrLbl.setStyle("-fx-font-size: 11px; -fx-text-fill: #8b949e;");

        VBox text = new VBox(2, nameLbl, addrLbl);

        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        Button playBtn = new Button("PLAY");
        playBtn.setStyle("-fx-background-color: #2da44e; -fx-text-fill: white; -fx-font-weight: bold; -fx-padding: 6 16;");
        playBtn.setOnAction(e -> launchServer(server.getName(), server.getAddress()));

        HBox card = new HBox(12, text, spacer, playBtn);
        card.setAlignment(Pos.CENTER_LEFT);
        card.setPadding(new Insets(12));
        card.setStyle("-fx-background-color: #161b22; -fx-border-color: #30363d; -fx-border-radius: 8; -fx-background-radius: 8;");
        return card;
    }

    private void populateRecommendedServers() {
        recommendedContainer.getChildren().clear();
        List<String[]> dummy = List.of(
                new String[]{"Hypixel Network", "mc.hypixel.net", "Popular Minigames & SkyBlock"},
                new String[]{"Wynncraft", "play.wynncraft.net", "The Minecraft MMORPG"},
                new String[]{"Zircon Official", "mc.zircon.example.com:25565", "Official Mod-Synced Server"}
        );

        for (String[] rec : dummy) {
            Label nameLbl = new Label(rec[0]);
            nameLbl.setStyle("-fx-font-size: 13px; -fx-font-weight: bold; -fx-text-fill: white;");

            Label descLbl = new Label(rec[2] + " (" + rec[1] + ")");
            descLbl.setStyle("-fx-font-size: 11px; -fx-text-fill: #8b949e;");

            VBox text = new VBox(2, nameLbl, descLbl);

            Region spacer = new Region();
            HBox.setHgrow(spacer, Priority.ALWAYS);

            Button joinBtn = new Button("Add & Play");
            joinBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-padding: 4 12; -fx-font-size: 11px;");
            joinBtn.setOnAction(e -> {
                SavedServer.recordPlayed(rec[0], rec[1]);
                populateServerList();
                launchServer(rec[0], rec[1]);
            });

            HBox card = new HBox(12, text, spacer, joinBtn);
            card.setAlignment(Pos.CENTER_LEFT);
            card.setPadding(new Insets(10, 12, 10, 12));
            card.setStyle("-fx-background-color: #0d1117; -fx-border-color: #21262d; -fx-border-radius: 8; -fx-background-radius: 8;");
            recommendedContainer.getChildren().add(card);
        }
    }

    private void promptAddServer() {
        TextInputDialog dialog = new TextInputDialog("localhost:25565");
        dialog.setTitle("Add Minecraft Server");
        dialog.setHeaderText("Connect to a Mod-Synced Minecraft Server");
        dialog.setContentText("Server Address (host:port):");

        Optional<String> result = dialog.showAndWait();
        result.ifPresent(addr -> {
            if (!addr.isBlank()) {
                SavedServer.recordPlayed("Custom Server", addr.trim());
                populateServerList();
                launchServer("Custom Server", addr.trim());
            }
        });
    }

    // ------------------------------------------------------------------
    // Skin Customizer
    // ------------------------------------------------------------------

    private void refreshSkinPreview() {
        Image customSkin = SkinManager.loadActiveSkinImage();
        if (customSkin != null) {
            skinPreview.setImage(customSkin);
            skinStatus.setText("Active Skin: Custom Upload (.PNG)");
        } else {
            skinPreview.setImage(null);
            skinStatus.setText("Active Skin: Default Steve / Alex");
        }
    }

    private void handleUploadSkin() {
        FileChooser chooser = new FileChooser();
        chooser.setTitle("Select Minecraft Skin PNG");
        chooser.getExtensionFilters().add(new FileChooser.ExtensionFilter("PNG Images", "*.png"));
        File selected = chooser.showOpenDialog(stage);
        if (selected != null) {
            try {
                SkinManager.saveSkin(selected);
                refreshSkinPreview();
                status("Uploaded new skin: " + selected.getName());
            } catch (IOException e) {
                log.warn("Failed to save custom skin", e);
                status("Failed to save skin: " + e.getMessage());
            }
        }
    }

    private void handleResetSkin() {
        SkinManager.resetSkin();
        refreshSkinPreview();
        status("Reset skin to default.");
    }

    // ------------------------------------------------------------------
    // Launch Pipeline
    // ------------------------------------------------------------------

    private void launchServer(String name, String serverAddress) {
        if (gameProcess != null && gameProcess.isAlive()) {
            gameProcess.destroy();
            status("Game process stopped.");
            gameProcess = null;
            return;
        }
        if (busy.compareAndSet(false, true)) {
            SavedServer.recordPlayed(name, serverAddress);
            populateServerList();
            setBusyUi(true);
            Thread.ofVirtual().name("launcher-flow").start(() -> runFlow(serverAddress));
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

            // 2. Gate on ownership: refuse to launch unless Mojang confirms the
            // session belongs to an account that owns Minecraft. checkEntitlements
            // only returns false when Mojang definitively rejects the token.
            if (!auth.checkEntitlements(session.getAccessToken())) {
                throw new IOException("Minecraft rejected this session — the account does not own "
                        + "Minecraft (Java Edition) or the session was revoked. "
                        + "Please sign in again with an account that owns the game.");
            }

            // 3. Parse the server address
            String[] hostPort = parseServerAddress(serverAddress);
            String host = hostPort[0];
            int port = Integer.parseInt(hostPort[1]);
            String baseUrl = "http://" + host + ":" + port;
            status("Server: " + baseUrl);

            Path gameDir = instanceGameDir(host, String.valueOf(port));
            Files.createDirectories(gameDir);

            // 4. Fetch BOM
            BillOfMaterials bom = fetchBom(baseUrl);
            ModLoaderInfo loader = bom.getModLoader();

            // 5. Resolve Launch Environment
            status("Resolving Minecraft " + bom.getMinecraftVersion() + " runtime...");
            int requiredJava = JavaRuntimeSelector.getRequiredJavaMajorVersion(bom.getMinecraftVersion());
            MinecraftClasspathBuilder.LaunchData launchData =
                    classpathBuilder.resolve(bom.getMinecraftVersion(), loader, requiredJava);

            // 6. Sync Mods using Staging Area & Reconciler
            status("Checking mod hashes & synchronizing staging area...");
            Platform.runLater(() -> progressBar.setVisible(true));
            boolean strict = strictVerifyCheck.isSelected();
            boolean trustDirect = trustDirectCheck.isSelected();

            ModSyncEngine.SyncResult syncResult = syncEngine.sync(baseUrl, gameDir, strict, trustDirect,
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

            // 7. Register the pre-join intent ticket (AGENT_PLAN_7): the server's
            // connection gate rejects Minecraft logins that carry no ticket, so
            // register ours before spawning the game. Best-effort — if this fails
            // the join will be refused by the server with a clear message.
            status("Registering pre-join intent with Zircon server...");
            registerPreJoinIntent(baseUrl, session.getUsername(), session.getUuid());

            // 8. Launch Game
            status("Starting Minecraft process...");
            gameProcess = runner.launch(launchData, session, gameDir, host, port, null);
            status("Game running — connected to " + host + ":" + port);
            Thread.ofVirtual().name("game-wait").start(() -> {
                try {
                    int code = gameProcess.waitFor();
                    gameProcess = null;
                    Platform.runLater(() -> status("Game exited (code " + code + ")."));
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            });
        } catch (Exception e) {
            log.error("Launcher flow failed", e);
            status("Error: " + describeError(e));
        } finally {
            Platform.runLater(() -> {
                busy.set(false);
                setBusyUi(false);
            });
        }
    }

    /**
     * Registers a short-lived join ticket with the server so the player's
     * connection passes the Zircon join gate. Best-effort: a failure here must
     * not abort the launch — the server's disconnect screen will surface it.
     */
    private void registerPreJoinIntent(String baseUrl, String username, String uuid) {
        try {
            java.net.http.HttpClient client = java.net.http.HttpClient.newBuilder()
                    .connectTimeout(java.time.Duration.ofSeconds(5))
                    .build();
            String json = BomJson.gson().toJson(Map.of(
                    "username", username == null ? "" : username,
                    "uuid", uuid == null ? "" : uuid));
            java.net.http.HttpRequest request = java.net.http.HttpRequest.newBuilder()
                    .uri(java.net.URI.create(baseUrl + "/api/join-intent"))
                    .header("Content-Type", "application/json")
                    .POST(java.net.http.HttpRequest.BodyPublishers.ofString(json))
                    .build();
            client.send(request, java.net.http.HttpResponse.BodyHandlers.discarding());
            log.debug("Pre-join ticket registered for {}", username);
        } catch (Exception e) {
            log.warn("Could not pre-register join ticket: {}", e.getMessage());
        }
    }

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

    private static Path instanceGameDir(String host, String portOrName) {
        String safeHost = host.replaceAll("[^A-Za-z0-9._-]", "_");
        return INSTANCES_ROOT.resolve(safeHost + "_" + portOrName);
    }

    private String[] parseServerAddress(String input) {
        String address = input == null ? "" : input.trim();
        if (address.isEmpty()) {
            return new String[]{"localhost", DEFAULT_SERVER_PORT};
        }
        String host = address;
        String port = DEFAULT_SERVER_PORT;
        if (address.startsWith("[")) {
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
    }

    private void status(String text) {
        Platform.runLater(() -> statusLabel.setText(text));
    }

    /**
     * Builds an actionable one-liner from an exception. When the message is null
     * (e.g. NullPointerException), falls back to the exception type and the first
     * stack frame so the UI never shows a bare "null".
     */
    private static String describeError(Throwable t) {
        if (t.getMessage() != null && !t.getMessage().isBlank()) {
            return t.getMessage();
        }
        StackTraceElement top = t.getStackTrace().length > 0 ? t.getStackTrace()[0] : null;
        return t.getClass().getSimpleName()
                + (top != null ? " at " + top.getClassName() + ":" + top.getLineNumber() : "");
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
}
