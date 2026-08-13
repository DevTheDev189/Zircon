package com.mcmanager.client.ui.controller;

import com.mcmanager.client.auth.MicrosoftAuthService;
import com.mcmanager.client.auth.SessionData;
import com.mcmanager.client.launch.JavaRuntimeSelector;
import com.mcmanager.client.launch.MinecraftClasspathBuilder;
import com.mcmanager.client.launch.MinecraftRunner;
import com.mcmanager.client.model.SavedServer;
import com.mcmanager.client.offline.OfflineInstance;
import com.mcmanager.client.offline.OfflineInstanceManager;
import com.mcmanager.client.pack.ClientPackManager;
import com.mcmanager.client.pack.PackSelection;
import com.mcmanager.client.skin.DefaultSkinFactory;
import com.mcmanager.client.skin.MojangSkinService;
import com.mcmanager.client.skin.SkinManager;
import com.mcmanager.client.sync.ModSyncEngine;
import com.mcmanager.client.sync.PackSyncEngine;
import com.mcmanager.client.ui.component.Player3DRenderer;
import com.mcmanager.core.api.ModrinthApiClient;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.BomJson;
import com.mcmanager.core.model.ModLoaderInfo;
import com.mcmanager.core.model.PackEntry;
import javafx.application.Platform;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Node;
import javafx.scene.control.Alert;
import javafx.scene.control.Button;
import javafx.scene.control.ButtonBar;
import javafx.scene.control.ButtonType;
import javafx.scene.control.CheckBox;
import javafx.scene.control.ComboBox;
import javafx.scene.control.Dialog;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.control.Slider;
import javafx.scene.control.TextField;
import javafx.scene.control.TextInputDialog;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.image.PixelReader;
import javafx.scene.input.DragEvent;
import javafx.scene.input.MouseEvent;
import javafx.scene.input.TransferMode;
import javafx.scene.layout.GridPane;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.TilePane;
import javafx.scene.layout.VBox;
import javafx.stage.FileChooser;
import javafx.stage.Stage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Controller driving the full Zircon launcher UI: Microsoft login overlay,
 * sidebar navigation, server sync &amp; launch, offline instance management,
 * skin history &amp; 3D preview, settings, and shader/texture pack syncing.
 */
public class MainController {

    private static final Logger log = LoggerFactory.getLogger(MainController.class);
    private static final String DEFAULT_SERVER_PORT = "25565";

    /** Gallery selection keys for the built-in default skins (not real files). */
    private static final String SKIN_DEFAULT_STEVE = "default:steve";
    private static final String SKIN_DEFAULT_ALEX = "default:alex";

    /** Offline packs combo value meaning "no shaderpack selected". */
    private static final String SHADERPACK_NONE = "None (shaders disabled)";

    private static final Path INSTANCES_ROOT = Path.of(
            System.getProperty("user.home"), ".zircon", "instances");

    private static final String NAV_STYLE = "-fx-font-size: 14px; -fx-padding: 10 14; -fx-background-radius: 8; "
            + "-fx-background-color: transparent; -fx-text-fill: #c9d1d9;";
    private static final String NAV_ACTIVE_STYLE = "-fx-font-size: 14px; -fx-padding: 10 14; -fx-background-radius: 8; "
            + "-fx-background-color: #21262d; -fx-text-fill: white; -fx-font-weight: bold;";

    // Login overlay & global chrome
    private final Button loginButton;
    private final Label loginStatus;
    private final Node loginView;
    private final Node mainLayout;
    private final Label userLabel;
    private final ImageView userAvatar;
    private final Button logoutButton;
    private final Label statusLabel;
    private final ProgressBar progressBar;
    private final Stage stage;

    // Navigation buttons & views
    private final Button navServers;
    private final Button navOffline;
    private final Button navSkins;
    private final Button navSettings;
    private final Node serverListView;
    private final Node offlineView;
    private final Node skinsView;
    private final Node settingsView;

    // Server list controls
    private final VBox savedServersContainer;
    private final VBox recommendedContainer;
    private final Button addServerBtn;

    // 3D player renderers
    private final Player3DRenderer serverRenderer;
    private final Player3DRenderer skinsRenderer;

    // Skin controls
    private final Button saveSkinBtn;
    private final Button removeSkinBtn;
    private final Label skinStatus;
    private final TilePane skinsGalleryContainer;

    // Offline instance controls
    private final VBox offlineInstancesContainer;
    private final Button newWorldBtn;
    private final Label offlineDetailTitle;
    private final Label offlineVersionLabel;
    private final Label offlineLoaderLabel;
    private final Label offlineLoaderVersionLabel;
    private final VBox offlineModsContainer;
    private final VBox offlineDropZone;
    private final TextField modrinthQuery;
    private final Button modrinthSearchBtn;
    private final VBox modrinthResultsContainer;
    private final ComboBox<String> offlineGameModeCombo;
    private final CheckBox offlineAllowCheatsCheck;
    private final Button offlinePlayBtn;
    private final Button offlineDeleteBtn;
    private final ComboBox<String> offlineShaderpackCombo;
    private final VBox offlineResourcepackContainer;
    private final Button offlineAddShaderpackBtn;
    private final Button offlineAddResourcepackBtn;

    // Settings controls
    private final Slider ramSlider;
    private final Label ramLabel;
    private final CheckBox strictVerifyCheck;
    private final CheckBox trustDirectCheck;
    private final TextField clientIdField;

    private final MicrosoftAuthService auth = new MicrosoftAuthService();
    private final ModSyncEngine syncEngine = new ModSyncEngine();
    private final PackSyncEngine packSyncEngine = new PackSyncEngine();
    private final MinecraftClasspathBuilder classpathBuilder = new MinecraftClasspathBuilder();
    private final MinecraftRunner runner = new MinecraftRunner();
    private final ModrinthApiClient modrinth = new ModrinthApiClient();
    private final HttpClient httpClient = HttpClient.newBuilder()
            .connectTimeout(java.time.Duration.ofSeconds(15))
            .followRedirects(HttpClient.Redirect.NORMAL)
            .build();

    private final AtomicBoolean busy = new AtomicBoolean(false);
    private volatile SessionData session;
    private volatile Process gameProcess;
    private volatile OfflineInstance selectedOfflineInstance;

    /** Gallery selection: a {@link Path} string or one of the {@code SKIN_DEFAULT_*} keys. */
    private volatile String selectedSkinKey;

    public MainController(Button loginButton, Label loginStatus, Node loginView, Node mainLayout,
                          Label userLabel, ImageView userAvatar, Button logoutButton,
                          Label statusLabel, ProgressBar progressBar, Stage stage,
                          Button navServers, Button navOffline, Button navSkins,
                          Button navSettings,
                          Node serverListView, Node offlineView, Node skinsView,
                          Node settingsView,
                          VBox savedServersContainer, VBox recommendedContainer, Button addServerBtn,
                          Player3DRenderer serverRenderer, Player3DRenderer skinsRenderer,
                          Button saveSkinBtn, Button removeSkinBtn, Label skinStatus,
                          TilePane skinsGalleryContainer,
                          VBox offlineInstancesContainer, Button newWorldBtn,
                          Label offlineDetailTitle, Label offlineVersionLabel,
                          Label offlineLoaderLabel, Label offlineLoaderVersionLabel,
                          VBox offlineModsContainer, VBox offlineDropZone,
                          TextField modrinthQuery, Button modrinthSearchBtn,
                          VBox modrinthResultsContainer, ComboBox<String> offlineGameModeCombo,
                          CheckBox offlineAllowCheatsCheck, Button offlinePlayBtn, Button offlineDeleteBtn,
                          ComboBox<String> offlineShaderpackCombo, VBox offlineResourcepackContainer,
                          Button offlineAddShaderpackBtn, Button offlineAddResourcepackBtn,
                          Slider ramSlider, Label ramLabel, CheckBox strictVerifyCheck,
                          CheckBox trustDirectCheck, TextField clientIdField) {
        this.loginButton = loginButton;
        this.loginStatus = loginStatus;
        this.loginView = loginView;
        this.mainLayout = mainLayout;
        this.userLabel = userLabel;
        this.userAvatar = userAvatar;
        this.logoutButton = logoutButton;
        this.statusLabel = statusLabel;
        this.progressBar = progressBar;
        this.stage = stage;

        this.navServers = navServers;
        this.navOffline = navOffline;
        this.navSkins = navSkins;
        this.navSettings = navSettings;
        this.serverListView = serverListView;
        this.offlineView = offlineView;
        this.skinsView = skinsView;
        this.settingsView = settingsView;

        this.savedServersContainer = savedServersContainer;
        this.recommendedContainer = recommendedContainer;
        this.addServerBtn = addServerBtn;

        this.serverRenderer = serverRenderer;
        this.skinsRenderer = skinsRenderer;

        this.saveSkinBtn = saveSkinBtn;
        this.removeSkinBtn = removeSkinBtn;
        this.skinStatus = skinStatus;
        this.skinsGalleryContainer = skinsGalleryContainer;

        this.offlineInstancesContainer = offlineInstancesContainer;
        this.newWorldBtn = newWorldBtn;
        this.offlineDetailTitle = offlineDetailTitle;
        this.offlineVersionLabel = offlineVersionLabel;
        this.offlineLoaderLabel = offlineLoaderLabel;
        this.offlineLoaderVersionLabel = offlineLoaderVersionLabel;
        this.offlineModsContainer = offlineModsContainer;
        this.offlineDropZone = offlineDropZone;
        this.modrinthQuery = modrinthQuery;
        this.modrinthSearchBtn = modrinthSearchBtn;
        this.modrinthResultsContainer = modrinthResultsContainer;
        this.offlineGameModeCombo = offlineGameModeCombo;
        this.offlineAllowCheatsCheck = offlineAllowCheatsCheck;
        this.offlinePlayBtn = offlinePlayBtn;
        this.offlineDeleteBtn = offlineDeleteBtn;
        this.offlineShaderpackCombo = offlineShaderpackCombo;
        this.offlineResourcepackContainer = offlineResourcepackContainer;
        this.offlineAddShaderpackBtn = offlineAddShaderpackBtn;
        this.offlineAddResourcepackBtn = offlineAddResourcepackBtn;

        this.ramSlider = ramSlider;
        this.ramLabel = ramLabel;
        this.strictVerifyCheck = strictVerifyCheck;
        this.trustDirectCheck = trustDirectCheck;
        this.clientIdField = clientIdField;
    }

    public void init() {
        navServers.setOnAction(e -> switchTab(serverListView, navServers));
        navOffline.setOnAction(e -> switchTab(offlineView, navOffline));
        navSkins.setOnAction(e -> switchTab(skinsView, navSkins));
        navSettings.setOnAction(e -> switchTab(settingsView, navSettings));
        switchTab(serverListView, navServers);

        // Login overlay & auth
        loginButton.setOnAction(e -> handleLogin());
        logoutButton.setOnAction(e -> onLogout());
        initSession();

        // Servers
        addServerBtn.setOnAction(e -> promptAddServer());
        populateServerList();
        populateRecommendedServers();

        // Skins
        refreshPlayerSkins();
        initSkinSelection();
        populateSkinsGallery();
        saveSkinBtn.setOnAction(e -> handleSaveSkin());
        removeSkinBtn.setOnAction(e -> handleRemoveSkin());

        // Offline instances
        populateOfflineInstances();
        newWorldBtn.setOnAction(e -> promptNewWorld());
        offlinePlayBtn.setOnAction(e -> handleOfflinePlay());
        offlineDeleteBtn.setOnAction(e -> handleOfflineDelete());
        offlineGameModeCombo.setOnAction(e -> persistOfflineSettings());
        offlineAllowCheatsCheck.setOnAction(e -> persistOfflineSettings());
        offlineDropZone.setOnMouseClicked(this::browseOfflineMods);
        offlineDropZone.setOnDragOver(this::onOfflineDragOver);
        offlineDropZone.setOnDragDropped(this::onOfflineDrop);
        modrinthSearchBtn.setOnAction(e -> handleModrinthSearch());
        modrinthQuery.setOnAction(e -> handleModrinthSearch());

        // Offline shaders & texture packs
        offlineShaderpackCombo.setOnAction(e -> persistOfflineShaderpack());
        offlineAddShaderpackBtn.setOnAction(e -> handleOfflineAddPack(true));
        offlineAddResourcepackBtn.setOnAction(e -> handleOfflineAddPack(false));

        // Settings
        ramSlider.valueProperty().addListener((obs, oldVal, newVal) ->
                ramLabel.setText("Max Memory Allocation (RAM): " + newVal.intValue() + " GB"));
    }

    // ------------------------------------------------------------------
    // Navigation & auth
    // ------------------------------------------------------------------

    private void switchTab(Node targetView, Button activeBtn) {
        serverListView.setVisible(targetView == serverListView);
        offlineView.setVisible(targetView == offlineView);
        skinsView.setVisible(targetView == skinsView);
        settingsView.setVisible(targetView == settingsView);

        for (Button btn : new Button[]{navServers, navOffline, navSkins, navSettings}) {
            btn.setStyle(btn == activeBtn ? NAV_ACTIVE_STYLE : NAV_STYLE);
        }

        if (targetView == serverListView) {
            refreshPlayerSkins();
        } else if (targetView == skinsView) {
            refreshPlayerSkins();
            populateSkinsGallery();
        } else if (targetView == offlineView) {
            populateOfflineInstances();
        }
    }

    private void initSession() {
        session = auth.loadCached();
        if (session != null) {
            onSessionEstablished();
        } else {
            showLoginView(true);
        }
    }

    private void showLoginView(boolean show) {
        loginView.setVisible(show);
    }

    private void handleLogin() {
        if (busy.compareAndSet(false, true)) {
            loginButton.setDisable(true);
            loginStatus.setText("Opening browser for Microsoft login...");
            setBusyUi(true);
            Thread.ofVirtual().name("login").start(() -> {
                try {
                    session = auth.login();
                    Platform.runLater(this::onSessionEstablished);
                } catch (Exception e) {
                    log.error("Microsoft login failed", e);
                    Platform.runLater(() -> {
                        loginStatus.setText("Login failed: " + describeError(e));
                        loginButton.setDisable(false);
                    });
                } finally {
                    Platform.runLater(() -> {
                        busy.set(false);
                        setBusyUi(false);
                    });
                }
            });
        }
    }

    private void onSessionEstablished() {
        userLabel.setText(session.getUsername());
        logoutButton.setVisible(true);
        loginButton.setDisable(false);
        loginStatus.setText("");
        showLoginView(false);
        status("Signed in as " + session.getUsername());
        refreshPlayerSkins();

        // Automatically sync the active Mojang skin in the background so the 3D
        // previews and sidebar always show the player's real skin.
        autoFetchMojangSkin();
    }

    /**
     * Downloads the player's active Mojang skin and applies it across all 3D
     * previews and the sidebar, with an indeterminate progress spinner while
     * fetching. Never blocks the FX thread.
     */
    private void autoFetchMojangSkin() {
        if (session == null || session.getUuid() == null || session.getUuid().isBlank()) {
            return;
        }
        status("Syncing active skin from Mojang...");
        progressBar.setVisible(true);
        progressBar.setProgress(ProgressBar.INDETERMINATE_PROGRESS);
        Thread.ofVirtual().name("mojang-auto-skin").start(() -> {
            try {
                MojangSkinService.DownloadedSkin skin = MojangSkinService.download(session.getUuid());
                // Sync straight to the active skin file — never archive into the
                // gallery history, or every launch/remove would add yet another
                // duplicate of the player's current skin.
                Path activeSkin = SkinManager.getActiveSkinPath();
                Files.createDirectories(activeSkin.getParent());
                Files.write(activeSkin, skin.png());
                Platform.runLater(() -> {
                    refreshPlayerSkins();
                    selectedSkinKey = activeSkin.toString();
                    updateSkinActionStates();
                    skinStatus.setText("Active Skin: Mojang (" + skin.variant() + ")");
                    status("Active Mojang skin synced.");
                    progressBar.setVisible(false);
                });
            } catch (Exception e) {
                log.warn("Could not auto-fetch Mojang skin: {}", e.getMessage());
                Platform.runLater(() -> {
                    refreshPlayerSkins();
                    progressBar.setVisible(false);
                });
            }
        });
    }

    public void shutdown() {
        if (gameProcess != null && gameProcess.isAlive()) {
            gameProcess.destroy();
        }
        com.mcmanager.client.render.GlContext.instance().dispose();
    }

    // ------------------------------------------------------------------
    // Server list management
    // ------------------------------------------------------------------

    private void populateServerList() {
        savedServersContainer.getChildren().clear();
        List<SavedServer> saved = SavedServer.load();
        if (saved.isEmpty()) {
            SavedServer.recordPlayed("Localhost Server", "localhost:25565");
            saved = SavedServer.load();
        }
        for (SavedServer s : saved) {
            savedServersContainer.getChildren().add(createSavedServerCard(s));
        }
    }

    private HBox createSavedServerCard(SavedServer server) {
        Label badge = serverBadge(server.getName());
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

        HBox card = new HBox(12, badge, text, spacer, playBtn);
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
            Label badge = serverBadge(rec[0]);
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

            HBox card = new HBox(12, badge, text, spacer, joinBtn);
            card.setAlignment(Pos.CENTER_LEFT);
            card.setPadding(new Insets(10, 12, 10, 12));
            card.setStyle("-fx-background-color: #0d1117; -fx-border-color: #21262d; -fx-border-radius: 8; -fx-background-radius: 8;");
            recommendedContainer.getChildren().add(card);
        }
    }

    private static Label serverBadge(String name) {
        String initial = name == null || name.isBlank() ? "?" : name.substring(0, 1).toUpperCase();
        Label badge = new Label(initial);
        badge.setMinSize(30, 30);
        badge.setMaxSize(30, 30);
        badge.setAlignment(Pos.CENTER);
        badge.setStyle("-fx-background-color: #2da44e; -fx-text-fill: white; -fx-font-weight: bold; "
                + "-fx-background-radius: 15; -fx-font-size: 13px;");
        return badge;
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
    // Skin preview, history & 3D binding
    // ------------------------------------------------------------------

    private void refreshPlayerSkins() {
        Image active = SkinManager.loadActiveSkinImage();
        if (active == null) {
            active = DefaultSkinFactory.steve();
        }
        serverRenderer.updateSkin(active);
        skinsRenderer.updateSkin(active);
        userAvatar.setImage(SkinManager.extractHeadIconScaled(active, 4));
    }

    private void populateSkinsGallery() {
        skinsGalleryContainer.getChildren().clear();
        skinsGalleryContainer.getChildren().add(skinCardAdd());
        skinsGalleryContainer.getChildren().add(skinCard(DefaultSkinFactory.steve(), "Steve",
                SKIN_DEFAULT_STEVE, () -> selectDefaultSkin(false)));
        skinsGalleryContainer.getChildren().add(skinCard(DefaultSkinFactory.alex(), "Alex",
                SKIN_DEFAULT_ALEX, () -> selectDefaultSkin(true)));
        for (Path path : SkinManager.getSkinHistory()) {
            Image skin = SkinManager.loadImage(path);
            if (skin == null) {
                continue;
            }
            skinsGalleryContainer.getChildren().add(skinCard(skin, path.getFileName().toString(),
                    path.toString(), () -> selectSkinFile(path)));
        }
        updateSelectionHighlight();
    }

    /** The "Add Skin" tile: imports a PNG into the gallery history and selects it. */
    private VBox skinCardAdd() {
        Label plus = new Label("+");
        plus.setStyle("-fx-font-size: 34px; -fx-text-fill: white; -fx-font-weight: bold;");
        Label caption = new Label("Add Skin");
        caption.setStyle("-fx-font-size: 11px; -fx-text-fill: #8b949e;");

        VBox card = new VBox(6, plus, caption);
        card.setAlignment(Pos.CENTER);
        card.setPrefSize(100, 116);
        card.setPadding(new Insets(10));
        card.setStyle(cardStyle(false));
        card.setOnMouseClicked(e -> handleAddSkin());
        return card;
    }

    /**
     * A selectable gallery card: head-icon preview, caption, and an emerald border
     * when it is the currently selected skin. Clicking previews only; the SAVE
     * button persists.
     */
    private VBox skinCard(Image skin, String label, String key, Runnable onClick) {
        ImageView view = new ImageView(SkinManager.extractHeadIconScaled(skin, 6));
        view.setFitWidth(64);
        view.setFitHeight(64);
        view.setPreserveRatio(true);
        view.setSmooth(false);

        Label caption = new Label(label);
        caption.setMaxWidth(96);
        caption.setStyle("-fx-font-size: 10px; -fx-text-fill: #8b949e; -fx-text-overrun: ellipsis;");

        VBox card = new VBox(6, view, caption);
        card.setAlignment(Pos.CENTER);
        card.setPrefSize(100, 116);
        card.setPadding(new Insets(10));
        card.setStyle(cardStyle(false));
        card.setUserData(key);
        card.setOnMouseClicked(e -> {
            selectedSkinKey = key;
            onClick.run();
            updateSelectionHighlight();
        });
        return card;
    }

    private static String cardStyle(boolean active) {
        return "-fx-background-color: #0d1117; -fx-border-color: " + (active ? "#2da44e" : "#21262d")
                + "; -fx-border-width: 2; -fx-border-radius: 10; -fx-background-radius: 10; -fx-cursor: hand;";
    }

    /** Applies the emerald selection border and the Remove button state for the selection. */
    private void updateSelectionHighlight() {
        for (Node card : skinsGalleryContainer.getChildren()) {
            if (card.getUserData() instanceof String key) {
                card.setStyle(cardStyle(key.equals(selectedSkinKey)));
            }
        }
        updateSkinActionStates();
    }

    /**
     * The Remove button is only usable when a non-default skin that is not the
     * currently active skin is selected.
     */
    private void updateSkinActionStates() {
        boolean removable = selectedSkinKey != null
                && !SKIN_DEFAULT_STEVE.equals(selectedSkinKey)
                && !SKIN_DEFAULT_ALEX.equals(selectedSkinKey)
                && !isActiveSkin(selectedSkinKey);
        removeSkinBtn.setDisable(!removable);
    }

    /** @return true when the given skin file is pixel-identical to the active skin. */
    private boolean isActiveSkin(String key) {
        Path file = Path.of(key);
        if (!Files.isRegularFile(file)) {
            return false;
        }
        return sameImage(SkinManager.loadImage(file), SkinManager.loadActiveSkinImage());
    }

    private static boolean sameImage(Image a, Image b) {
        if (a == b) {
            return true;
        }
        if (a == null || b == null) {
            return false;
        }
        if (a.getWidth() != b.getWidth() || a.getHeight() != b.getHeight()) {
            return false;
        }
        PixelReader ra = a.getPixelReader();
        PixelReader rb = b.getPixelReader();
        for (int y = 0; y < a.getHeight(); y++) {
            for (int x = 0; x < a.getWidth(); x++) {
                if (ra.getArgb(x, y) != rb.getArgb(x, y)) {
                    return false;
                }
            }
        }
        return true;
    }

    /** Pre-selects the active skin (or Steve) when the gallery first opens. */
    private void initSkinSelection() {
        selectedSkinKey = SkinManager.hasCustomSkin()
                ? SkinManager.getActiveSkinPath().toString()
                : SKIN_DEFAULT_STEVE;
    }

    private void applyDefaultSkin(boolean alex) {
        SkinManager.resetSkin();
        Image image = alex ? DefaultSkinFactory.alex() : DefaultSkinFactory.steve();
        serverRenderer.updateSkin(image);
        skinsRenderer.updateSkin(image);
        userAvatar.setImage(SkinManager.extractHeadIconScaled(image, 4));
        skinStatus.setText("Active Skin: Default " + (alex ? "Alex" : "Steve"));
    }

    /** Previews a default skin in all previews without persisting (SAVE commits). */
    private void selectDefaultSkin(boolean alex) {
        Image image = alex ? DefaultSkinFactory.alex() : DefaultSkinFactory.steve();
        serverRenderer.updateSkin(image);
        skinsRenderer.updateSkin(image);
        skinStatus.setText("Preview: Default " + (alex ? "Alex" : "Steve") + " — press SAVE to activate.");
    }

    /** Previews a skin file in all previews without persisting (SAVE commits). */
    private void selectSkinFile(Path path) {
        Image image = SkinManager.loadImage(path);
        if (image == null) {
            return;
        }
        serverRenderer.updateSkin(image);
        skinsRenderer.updateSkin(image);
        skinStatus.setText("Preview: " + path.getFileName() + " — press SAVE to activate.");
    }

    /**
     * Removes the selected skin from the gallery list and reverts the player to
     * their Mojang skin. The currently active skin can never be removed (the
     * button is disabled for it), so the active skin file stays consistent.
     */
    private void handleRemoveSkin() {
        if (selectedSkinKey == null
                || SKIN_DEFAULT_STEVE.equals(selectedSkinKey)
                || SKIN_DEFAULT_ALEX.equals(selectedSkinKey)
                || isActiveSkin(selectedSkinKey)) {
            return;
        }
        Path file = Path.of(selectedSkinKey);
        try {
            Files.deleteIfExists(file);
        } catch (IOException e) {
            status("Failed to remove skin: " + e.getMessage());
            return;
        }
        status("Removed skin: " + file.getFileName());
        populateSkinsGallery();

        // Revert the player to their Mojang skin; when signed out, just restore
        // the preview to the current active skin.
        if (session != null && session.getUuid() != null && !session.getUuid().isBlank()) {
            autoFetchMojangSkin();
        } else {
            refreshPlayerSkins();
            status("Skin removed — sign in to sync your Mojang skin.");
        }
    }

    /**
     * SAVE: persists the selected skin locally and, when signed in, uploads it to
     * Mojang so it follows the player everywhere. Default selections just reset
     * the local active skin (there is nothing to upload).
     */
    private void handleSaveSkin() {
        if (selectedSkinKey == null) {
            status("Select a skin first.");
            return;
        }
        if (SKIN_DEFAULT_STEVE.equals(selectedSkinKey)) {
            applyDefaultSkin(false);
            updateSkinActionStates();
            return;
        }
        if (SKIN_DEFAULT_ALEX.equals(selectedSkinKey)) {
            applyDefaultSkin(true);
            updateSkinActionStates();
            return;
        }
        Path file = Path.of(selectedSkinKey);
        if (!Files.isRegularFile(file)) {
            status("Selected skin file is missing: " + file.getFileName());
            return;
        }
        saveAndUploadSkin(file);
        updateSkinActionStates();
    }

    /** Copies the selected skin to the active local skin, then uploads to Mojang. */
    private void saveAndUploadSkin(Path file) {
        try {
            Files.createDirectories(SkinManager.getActiveSkinPath().getParent());
            Files.copy(file, SkinManager.getActiveSkinPath(), StandardCopyOption.REPLACE_EXISTING);
            refreshPlayerSkins();
            skinStatus.setText("Active Skin: " + file.getFileName());
            status("Skin saved locally.");
        } catch (IOException e) {
            log.warn("Failed to save skin", e);
            status("Failed to save skin: " + e.getMessage());
            return;
        }

        if (session == null || session.getAccessToken() == null || session.getAccessToken().isBlank()) {
            status("Skin saved locally — sign in to also upload it to Mojang.");
            return;
        }
        Thread.ofVirtual().name("mojang-skin-save").start(() -> {
            try {
                SessionData fresh = ensureFreshSession();
                MojangSkinService.upload(fresh.getAccessToken(), file, "classic");
                Platform.runLater(() -> status("Skin saved & uploaded to Mojang."));
            } catch (Exception e) {
                log.warn("Mojang skin upload failed", e);
                Platform.runLater(() -> status("Saved locally, but Mojang upload failed: " + describeError(e)));
            }
        });
    }

    /** Add Skin card: imports a PNG into the gallery history and selects it. */
    private void handleAddSkin() {
        FileChooser chooser = new FileChooser();
        chooser.setTitle("Select Minecraft Skin PNG");
        chooser.getExtensionFilters().add(new FileChooser.ExtensionFilter("PNG Images", "*.png"));
        File selected = chooser.showOpenDialog(stage);
        if (selected == null) {
            return;
        }
        try {
            // Import into the gallery history without activating; SAVE commits.
            SkinManager.saveToHistory(selected);
            List<Path> history = SkinManager.getSkinHistory();
            if (!history.isEmpty()) {
                Path archived = history.get(0); // newest entry is the one just added
                selectedSkinKey = archived.toString();
                selectSkinFile(archived);
            }
            populateSkinsGallery();
            status("Skin added to gallery — press SAVE to activate & upload to Mojang.");
        } catch (IOException e) {
            log.warn("Failed to add skin", e);
            status("Failed to add skin: " + e.getMessage());
        }
    }

    /**
     * Returns a valid session, signing in or silently refreshing the Microsoft /
     * Minecraft tokens if the cached one is missing or expired.
     */
    private SessionData ensureFreshSession() throws IOException, InterruptedException {
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
        return session;
    }

    // ------------------------------------------------------------------
    // Offline instances
    // ------------------------------------------------------------------

    private void populateOfflineInstances() {
        offlineInstancesContainer.getChildren().clear();
        List<OfflineInstance> instances = OfflineInstanceManager.loadAll();
        for (OfflineInstance instance : instances) {
            offlineInstancesContainer.getChildren().add(createOfflineInstanceCard(instance));
        }

        if (instances.isEmpty()) {
            renderOfflineDetail(null);
            return;
        }

        OfflineInstance toSelect = instances.get(0);
        if (selectedOfflineInstance != null) {
            for (OfflineInstance instance : instances) {
                if (instance.getId().equals(selectedOfflineInstance.getId())) {
                    toSelect = instance;
                    break;
                }
            }
        }
        renderOfflineDetail(toSelect);
        updateInstanceCardHighlight();
    }

    private HBox createOfflineInstanceCard(OfflineInstance instance) {
        Label nameLbl = new Label(instance.getName());
        nameLbl.setStyle("-fx-font-size: 14px; -fx-font-weight: bold; -fx-text-fill: white;");

        Label versionBadge = new Label(instance.getMinecraftVersion());
        versionBadge.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; "
                + "-fx-font-size: 10px; -fx-padding: 2 6; -fx-background-radius: 6;");

        Label gameModeBadge = new Label(defaultString(instance.getGameMode(), "survival"));
        gameModeBadge.setStyle("-fx-background-color: #21262d; -fx-text-fill: #2da44e; "
                + "-fx-font-size: 10px; -fx-padding: 2 6; -fx-background-radius: 6;");

        HBox badges = new HBox(6, versionBadge, gameModeBadge);
        VBox text = new VBox(4, nameLbl, badges);

        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        Button playBtn = new Button("PLAY");
        playBtn.setStyle("-fx-background-color: #2da44e; -fx-text-fill: white; -fx-font-weight: bold; -fx-padding: 4 12; -fx-font-size: 11px;");
        playBtn.setOnAction(e -> {
            renderOfflineDetail(instance);
            updateInstanceCardHighlight();
            handleOfflinePlay();
        });

        HBox card = new HBox(12, text, spacer, playBtn);
        card.setAlignment(Pos.CENTER_LEFT);
        card.setPadding(new Insets(12));
        card.setStyle(offlineCardStyle(false));
        card.setUserData(instance.getId());
        card.setOnMouseClicked(e -> {
            renderOfflineDetail(instance);
            updateInstanceCardHighlight();
        });
        return card;
    }

    private static String offlineCardStyle(boolean active) {
        return "-fx-background-color: #161b22; -fx-border-color: " + (active ? "#2da44e" : "#30363d")
                + "; -fx-border-width: 2; -fx-border-radius: 8; -fx-background-radius: 8; -fx-cursor: hand;";
    }

    /** Rings the card of the currently selected offline instance. */
    private void updateInstanceCardHighlight() {
        String selectedId = selectedOfflineInstance == null ? null : selectedOfflineInstance.getId();
        for (Node card : offlineInstancesContainer.getChildren()) {
            if (card.getUserData() instanceof String id) {
                card.setStyle(offlineCardStyle(id.equals(selectedId)));
            }
        }
    }

    private void renderOfflineDetail(OfflineInstance instance) {
        selectedOfflineInstance = instance;
        if (instance == null) {
            offlineDetailTitle.setText("Select a world");
            offlineVersionLabel.setText("Minecraft: —");
            offlineLoaderLabel.setText("Loader: —");
            offlineLoaderVersionLabel.setText("Loader version: —");
            offlineModsContainer.getChildren().clear();
            offlineGameModeCombo.setValue("survival");
            offlineAllowCheatsCheck.setSelected(false);
            offlineShaderpackCombo.getItems().clear();
            offlineShaderpackCombo.setValue(null);
            offlineResourcepackContainer.getChildren().clear();
            offlinePlayBtn.setDisable(true);
            offlineDeleteBtn.setDisable(true);
            return;
        }

        offlineDetailTitle.setText(instance.getName());
        offlineVersionLabel.setText("Minecraft: " + instance.getMinecraftVersion());
        offlineLoaderLabel.setText("Loader: " + instance.getModLoader().getType());
        offlineLoaderVersionLabel.setText("Loader version: " + defaultString(instance.getModLoader().getVersion(), ""));
        offlineGameModeCombo.setValue(defaultString(instance.getGameMode(), "survival"));
        offlineAllowCheatsCheck.setSelected(instance.isAllowCheats());
        offlinePlayBtn.setDisable(false);
        offlineDeleteBtn.setDisable(false);
        renderOfflineMods(instance);
        renderOfflinePacks(instance);
    }

    private void renderOfflineMods(OfflineInstance instance) {
        offlineModsContainer.getChildren().clear();
        List<Path> mods = OfflineInstanceManager.listMods(instance);
        if (mods.isEmpty()) {
            offlineModsContainer.getChildren().add(infoLabel("No mods yet — drop files or search Modrinth."));
            return;
        }
        for (Path mod : mods) {
            Label name = new Label(mod.getFileName().toString());
            name.setStyle("-fx-font-size: 12px; -fx-text-fill: #c9d1d9;");
            offlineModsContainer.getChildren().add(name);
        }
    }

    private void persistOfflineSettings() {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance == null) {
            return;
        }
        instance.setGameMode(offlineGameModeCombo.getValue());
        instance.setAllowCheats(offlineAllowCheatsCheck.isSelected());
        try {
            OfflineInstanceManager.save(instance);
        } catch (IOException e) {
            status("Could not save world settings: " + e.getMessage());
        }
    }

    /**
     * Fills the offline shaderpack combo and texture-pack checkboxes from the
     * instance's {@code pack-selection.json} and local pack folders.
     */
    private void renderOfflinePacks(OfflineInstance instance) {
        Path gameDir = OfflineInstanceManager.instanceDir(instance.getId());
        PackSelection selection = PackSelection.load(gameDir);

        offlineShaderpackCombo.getItems().clear();
        offlineShaderpackCombo.getItems().add(SHADERPACK_NONE);
        offlineShaderpackCombo.getItems().addAll(listPackFiles(gameDir.resolve("shaderpacks")));
        boolean shadersOn = selection.isShadersEnabled() && selection.getActiveShaderpack() != null;
        offlineShaderpackCombo.setValue(shadersOn ? selection.getActiveShaderpack() : SHADERPACK_NONE);

        offlineResourcepackContainer.getChildren().clear();
        List<String> packs = listPackFiles(gameDir.resolve("resourcepacks"));
        if (packs.isEmpty()) {
            offlineResourcepackContainer.getChildren().add(infoLabel("No texture packs added."));
        } else {
            for (String filename : packs) {
                CheckBox cb = new CheckBox(filename);
                cb.setStyle("-fx-text-fill: #c9d1d9; -fx-font-size: 12px;");
                cb.setSelected(selection.getActiveResourcepacks().contains(filename));
                cb.setOnAction(e -> {
                    if (cb.isSelected()) {
                        if (!selection.getActiveResourcepacks().contains(filename)) {
                            selection.getActiveResourcepacks().add(filename);
                        }
                    } else {
                        selection.getActiveResourcepacks().remove(filename);
                    }
                    selection.save(gameDir);
                });
                offlineResourcepackContainer.getChildren().add(cb);
            }
        }
    }

    /** Persists the offline shaderpack combo choice to {@code pack-selection.json}. */
    private void persistOfflineShaderpack() {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance == null) {
            return;
        }
        Path gameDir = OfflineInstanceManager.instanceDir(instance.getId());
        PackSelection selection = PackSelection.load(gameDir);
        String value = offlineShaderpackCombo.getValue();
        if (value == null || SHADERPACK_NONE.equals(value)) {
            selection.setShadersEnabled(false);
            selection.setActiveShaderpack(null);
        } else {
            selection.setShadersEnabled(true);
            selection.setActiveShaderpack(value);
        }
        selection.save(gameDir);
    }

    /** Adds local {@code .zip} shaderpacks/resourcepacks to the selected offline world. */
    private void handleOfflineAddPack(boolean shader) {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance == null) {
            status("Select a world first.");
            return;
        }
        FileChooser chooser = new FileChooser();
        chooser.setTitle(shader ? "Select Shaderpack (.zip)" : "Select Texture Pack (.zip)");
        chooser.getExtensionFilters().add(new FileChooser.ExtensionFilter("ZIP Archives", "*.zip"));
        List<File> files = chooser.showOpenMultipleDialog(stage);
        if (files == null || files.isEmpty()) {
            return;
        }
        try {
            Path gameDir = OfflineInstanceManager.instanceDir(instance.getId());
            PackSelection selection = PackSelection.load(gameDir);
            for (File file : files) {
                if (shader) {
                    ClientPackManager.addLocalShaderpack(gameDir, file, selection);
                } else {
                    ClientPackManager.addLocalResourcepack(gameDir, file, selection);
                }
            }
            renderOfflinePacks(instance);
            status("Added " + files.size() + " local " + (shader ? "shaderpack(s)." : "texture pack(s)."));
        } catch (IOException e) {
            status("Failed to add local pack: " + e.getMessage());
        }
    }

    private void promptNewWorld() {
        Dialog<OfflineInstance> dialog = new Dialog<>();
        dialog.setTitle("New Offline World");
        dialog.setHeaderText("Create a new offline Minecraft world");

        ButtonType createType = new ButtonType("Create", ButtonBar.ButtonData.OK_DONE);
        dialog.getDialogPane().getButtonTypes().addAll(createType, ButtonType.CANCEL);

        GridPane grid = new GridPane();
        grid.setHgap(10);
        grid.setVgap(10);
        grid.setPadding(new Insets(20));

        TextField nameField = new TextField("My World");
        TextField versionField = new TextField("1.20.4");
        ComboBox<String> loaderCombo = new ComboBox<>();
        loaderCombo.getItems().addAll("fabric", "forge", "neoforge", "quilt");
        loaderCombo.setValue("fabric");
        TextField loaderVersionField = new TextField("0.15.11");

        grid.add(new Label("Name:"), 0, 0);
        grid.add(nameField, 1, 0);
        grid.add(new Label("Minecraft:"), 0, 1);
        grid.add(versionField, 1, 1);
        grid.add(new Label("Loader:"), 0, 2);
        grid.add(loaderCombo, 1, 2);
        grid.add(new Label("Loader version:"), 0, 3);
        grid.add(loaderVersionField, 1, 3);
        dialog.getDialogPane().setContent(grid);

        dialog.setResultConverter(btn -> {
            if (btn == createType) {
                OfflineInstance draft = new OfflineInstance();
                draft.setName(nameField.getText());
                draft.setMinecraftVersion(versionField.getText());
                draft.setModLoader(new ModLoaderInfo(
                        loaderCombo.getValue(), loaderVersionField.getText(), ""));
                return draft;
            }
            return null;
        });

        dialog.showAndWait().ifPresent(draft -> {
            try {
                OfflineInstance instance = OfflineInstanceManager.createInstance(
                        draft.getName(), draft.getMinecraftVersion(),
                        draft.getModLoader().getType(), draft.getModLoader().getVersion());
                selectedOfflineInstance = instance;
                populateOfflineInstances();
                status("Created " + instance.getName());
            } catch (IOException e) {
                status("Failed to create world: " + e.getMessage());
            }
        });
    }

    private void handleOfflinePlay() {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance == null) {
            status("Select a world first.");
            return;
        }
        if (gameProcess != null && gameProcess.isAlive()) {
            gameProcess.destroy();
            status("Game process stopped.");
            gameProcess = null;
            return;
        }
        persistOfflineSettings();
        if (busy.compareAndSet(false, true)) {
            setBusyUi(true);
            Thread.ofVirtual().name("offline-launch").start(() -> launchOfflineFlow(instance));
        }
    }

    private void launchOfflineFlow(OfflineInstance instance) {
        try {
            status("Resolving Minecraft " + instance.getMinecraftVersion() + " runtime...");
            int requiredJava = JavaRuntimeSelector.getRequiredJavaMajorVersion(instance.getMinecraftVersion());
            MinecraftClasspathBuilder.LaunchData launchData =
                    classpathBuilder.resolve(instance.getMinecraftVersion(), instance.getModLoader(), requiredJava);

            Path gameDir = OfflineInstanceManager.instanceDir(instance.getId());
            Files.createDirectories(gameDir);

            status("Starting offline world '" + instance.getName() + "'...");
            String playerName = (session != null && session.getUsername() != null && !session.getUsername().isBlank())
                    ? session.getUsername() : "Player";
            gameProcess = runner.launchOffline(launchData, playerName, instance.getJavaArgs(), gameDir, null);

            instance.setLastPlayed(System.currentTimeMillis());
            try {
                OfflineInstanceManager.save(instance);
            } catch (IOException ignored) {
                // Best-effort last-played stamp.
            }

            status("Playing " + instance.getName() + " (offline).");
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
            log.error("Offline launch failed", e);
            status("Error: " + describeError(e));
        } finally {
            Platform.runLater(() -> {
                busy.set(false);
                setBusyUi(false);
            });
        }
    }

    private void handleOfflineDelete() {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance == null) {
            return;
        }
        Alert confirm = new Alert(Alert.AlertType.CONFIRMATION,
                "Delete '" + instance.getName() + "' and all of its files?", ButtonType.YES, ButtonType.NO);
        confirm.setTitle("Delete Offline World");
        confirm.setHeaderText(null);
        confirm.showAndWait().ifPresent(btn -> {
            if (btn == ButtonType.YES) {
                OfflineInstanceManager.delete(instance);
                selectedOfflineInstance = null;
                populateOfflineInstances();
                status("Deleted " + instance.getName());
            }
        });
    }

    // ------------------------------------------------------------------
    // Offline mods: drag-and-drop + Modrinth
    // ------------------------------------------------------------------

    private void onOfflineDragOver(DragEvent event) {
        if (event.getDragboard().hasFiles()) {
            event.acceptTransferModes(TransferMode.COPY);
        }
        event.consume();
    }

    private void onOfflineDrop(DragEvent event) {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance != null && event.getDragboard().hasFiles()) {
            List<File> jars = event.getDragboard().getFiles().stream()
                    .filter(f -> f.getName().toLowerCase().endsWith(".jar"))
                    .toList();
            copyMods(instance, jars);
            event.setDropCompleted(!jars.isEmpty());
        }
        event.consume();
    }

    private void browseOfflineMods(MouseEvent event) {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance == null) {
            status("Select a world first.");
            return;
        }
        FileChooser chooser = new FileChooser();
        chooser.setTitle("Select Minecraft Mods (.jar)");
        chooser.getExtensionFilters().add(new FileChooser.ExtensionFilter("JAR Files", "*.jar"));
        List<File> selected = chooser.showOpenMultipleDialog(stage);
        if (selected != null) {
            copyMods(instance, selected);
        }
    }

    private void copyMods(OfflineInstance instance, List<File> files) {
        if (instance == null || files == null || files.isEmpty()) {
            return;
        }
        try {
            Path modsDir = OfflineInstanceManager.modsDir(instance);
            Files.createDirectories(modsDir);
            int count = 0;
            for (File file : files) {
                if (!file.getName().toLowerCase().endsWith(".jar")) {
                    continue;
                }
                Files.copy(file.toPath(), modsDir.resolve(file.getName()), StandardCopyOption.REPLACE_EXISTING);
                count++;
            }
            renderOfflineMods(instance);
            status("Added " + count + " mod(s) to " + instance.getName());
        } catch (IOException e) {
            status("Failed to add mods: " + e.getMessage());
        }
    }

    private void handleModrinthSearch() {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance == null) {
            status("Select a world first.");
            return;
        }
        String query = modrinthQuery.getText();
        if (query == null || query.isBlank()) {
            status("Enter a mod name to search.");
            return;
        }
        modrinthSearchBtn.setDisable(true);
        modrinthResultsContainer.getChildren().clear();
        modrinthResultsContainer.getChildren().add(infoLabel("Searching Modrinth..."));
        Thread.ofVirtual().name("modrinth-search").start(() -> {
            try {
                List<ModrinthApiClient.ModrinthSearchHit> hits = modrinth.searchMods(
                        query.trim(), instance.getMinecraftVersion(),
                        instance.getModLoader().getType(), "mod");
                Platform.runLater(() -> renderModrinthResults(hits));
            } catch (Exception e) {
                log.warn("Modrinth search failed", e);
                Platform.runLater(() -> {
                    modrinthResultsContainer.getChildren().clear();
                    modrinthResultsContainer.getChildren().add(infoLabel("Search failed: " + describeError(e)));
                    modrinthSearchBtn.setDisable(false);
                });
            }
        });
    }

    private void renderModrinthResults(List<ModrinthApiClient.ModrinthSearchHit> hits) {
        modrinthResultsContainer.getChildren().clear();
        modrinthSearchBtn.setDisable(false);
        if (hits.isEmpty()) {
            modrinthResultsContainer.getChildren().add(infoLabel("No results found."));
            return;
        }
        for (ModrinthApiClient.ModrinthSearchHit hit : hits) {
            Label title = new Label(hit.title);
            title.setStyle("-fx-font-size: 12px; -fx-font-weight: bold; -fx-text-fill: white;");

            Label desc = new Label(hit.description == null ? "" : hit.description);
            desc.setMaxWidth(300);
            desc.setWrapText(true);
            desc.setStyle("-fx-font-size: 10px; -fx-text-fill: #8b949e;");

            VBox text = new VBox(2, title, desc);

            Region spacer = new Region();
            HBox.setHgrow(spacer, Priority.ALWAYS);

            Button installBtn = new Button("Install");
            installBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-padding: 4 10; -fx-font-size: 11px;");
            installBtn.setOnAction(e -> installModrinthMod(hit));

            HBox row = new HBox(10, text, spacer, installBtn);
            row.setAlignment(Pos.CENTER_LEFT);
            row.setPadding(new Insets(8));
            row.setStyle("-fx-background-color: #0d1117; -fx-border-color: #21262d; -fx-border-radius: 6; -fx-background-radius: 6;");
            modrinthResultsContainer.getChildren().add(row);
        }
    }

    private void installModrinthMod(ModrinthApiClient.ModrinthSearchHit hit) {
        OfflineInstance instance = selectedOfflineInstance;
        if (instance == null) {
            status("Select a world first.");
            return;
        }
        status("Installing " + hit.title + "...");
        Thread.ofVirtual().name("modrinth-install").start(() -> {
            try {
                List<ModrinthApiClient.ModrinthVersion> versions = modrinth.listProjectVersions(
                        hit.projectId, instance.getMinecraftVersion(), instance.getModLoader().getType());
                if (versions.isEmpty()) {
                    Platform.runLater(() -> status("No compatible version of " + hit.title + " found."));
                    return;
                }
                ModrinthApiClient.ModrinthVersion version = versions.get(0);
                ModrinthApiClient.ModrinthFile file = version.primaryFile();
                if (file == null || file.url == null || file.url.isBlank()) {
                    Platform.runLater(() -> status("No downloadable file for " + hit.title + "."));
                    return;
                }
                String filename = file.filename == null || file.filename.isBlank()
                        ? hit.title + ".jar" : file.filename;
                Path dest = OfflineInstanceManager.modsDir(instance).resolve(filename);
                downloadFile(file.url, dest);
                Platform.runLater(() -> {
                    renderOfflineMods(instance);
                    status("Installed " + filename);
                });
            } catch (Exception e) {
                log.warn("Modrinth install failed", e);
                Platform.runLater(() -> status("Install failed: " + describeError(e)));
            }
        });
    }

    private void downloadFile(String url, Path dest) throws IOException, InterruptedException {
        Path tmp = Files.createTempFile("modrinth-", ".jar");
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .header("User-Agent", ModrinthApiClient.DEFAULT_USER_AGENT)
                .GET()
                .build();
        HttpResponse<Path> response = httpClient.send(request, HttpResponse.BodyHandlers.ofFile(tmp));
        if (response.statusCode() / 100 != 2) {
            Files.deleteIfExists(tmp);
            throw new IOException("Download failed: HTTP " + response.statusCode());
        }
        Files.createDirectories(dest.getParent());
        Files.copy(tmp, dest, StandardCopyOption.REPLACE_EXISTING);
        Files.deleteIfExists(tmp);
    }

    private List<String> listPackFiles(Path dir) {
        try (var stream = Files.list(dir)) {
            return stream.filter(Files::isRegularFile)
                    .map(p -> p.getFileName().toString())
                    .filter(n -> n.toLowerCase().endsWith(".zip"))
                    .sorted()
                    .toList();
        } catch (IOException e) {
            return List.of();
        }
    }

    // ------------------------------------------------------------------
    // Server launch pipeline
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

    /** @return true when the server's BOM advertises shaderpacks or texture packs. */
    private static boolean bomOffersPacks(BillOfMaterials bom) {
        return (bom.getShaderpacks() != null && !bom.getShaderpacks().isEmpty())
                || (bom.getResourcepacks() != null && !bom.getResourcepacks().isEmpty());
    }

    /**
     * Shows the "Server Recommended Packs" confirmation on the FX thread and
     * blocks the caller until the player answers. The dialog runs in a nested
     * JavaFX event loop, so the UI stays responsive while the caller waits.
     */
    private boolean promptForServerPacksBlocking(BillOfMaterials bom) {
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<Boolean> choice = new AtomicReference<>(false);
        Platform.runLater(() -> {
            try {
                choice.set(promptUserForServerPacks(bom));
            } finally {
                latch.countDown();
            }
        });
        try {
            latch.await();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        return choice.get();
    }

    private boolean promptUserForServerPacks(BillOfMaterials bom) {
        List<String> packNames = new ArrayList<>();
        for (PackEntry pack : bom.getShaderpacks()) {
            packNames.add(pack.getTitle() != null ? pack.getTitle() : pack.getFilename());
        }
        for (PackEntry pack : bom.getResourcepacks()) {
            packNames.add(pack.getTitle() != null ? pack.getTitle() : pack.getFilename());
        }

        Alert dialog = new Alert(Alert.AlertType.CONFIRMATION);
        dialog.setTitle("Server Recommended Packs");
        dialog.setHeaderText(null);
        dialog.setContentText("This server recommends the following shader & texture packs "
                + "for optimal gameplay: " + String.join(", ", packNames)
                + ". Would you like to download and enable them?");
        ButtonType enable = new ButtonType("Enable & Sync", ButtonBar.ButtonData.YES);
        ButtonType vanilla = new ButtonType("Play Vanilla/No Packs", ButtonBar.ButtonData.NO);
        dialog.getButtonTypes().setAll(enable, vanilla);
        Optional<ButtonType> result = dialog.showAndWait();
        return result.orElse(vanilla) == enable;
    }

    private void runFlow(String serverAddress) {
        try {
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

            if (!auth.checkEntitlements(session.getAccessToken())) {
                throw new IOException("Minecraft rejected this session — the account does not own "
                        + "Minecraft (Java Edition) or the session was revoked. "
                        + "Please sign in again with an account that owns the game.");
            }

            String[] hostPort = parseServerAddress(serverAddress);
            String host = hostPort[0];
            int port = Integer.parseInt(hostPort[1]);
            String baseUrl = "http://" + host + ":" + port;
            status("Server: " + baseUrl);

            Path gameDir = instanceGameDir(host, String.valueOf(port));
            Files.createDirectories(gameDir);

            BillOfMaterials bom = fetchBom(baseUrl);
            ModLoaderInfo loader = bom.getModLoader();

            // Server-driven pack sync: if the server recommends shaderpacks or
            // texture packs, ask the player before downloading & enabling them.
            if (bomOffersPacks(bom)) {
                boolean enablePacks = promptForServerPacksBlocking(bom);
                if (enablePacks) {
                    status("Syncing server shaderpacks & texture packs...");
                    PackSelection selection = PackSelection.load(gameDir);
                    packSyncEngine.sync(bom, baseUrl, gameDir,
                            selection.getLocallyAddedShaderpacks(),
                            selection.getLocallyAddedResourcepacks(),
                            msg -> status(msg));

                    selection.setShadersEnabled(true);
                    if (!bom.getShaderpacks().isEmpty()) {
                        selection.setActiveShaderpack(bom.getShaderpacks().get(0).getFilename());
                    }
                    selection.setActiveResourcepacks(bom.getResourcepacks().stream()
                            .map(PackEntry::getFilename).toList());
                    selection.save(gameDir);
                }
            }

            status("Resolving Minecraft " + bom.getMinecraftVersion() + " runtime...");
            int requiredJava = JavaRuntimeSelector.getRequiredJavaMajorVersion(bom.getMinecraftVersion());
            MinecraftClasspathBuilder.LaunchData launchData =
                    classpathBuilder.resolve(bom.getMinecraftVersion(), loader, requiredJava);

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

            status("Registering pre-join intent with Zircon server...");
            registerPreJoinIntent(baseUrl, session.getUsername(), session.getUuid());

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
        return BomJson.fromJson(response.body());
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
        userAvatar.setImage(null);
        logoutButton.setVisible(false);
        showLoginView(true);
        status("Signed out.");
    }

    // ------------------------------------------------------------------
    // UI helpers
    // ------------------------------------------------------------------

    private void status(String text) {
        Platform.runLater(() -> statusLabel.setText(text));
    }

    private Label infoLabel(String text) {
        Label label = new Label(text);
        label.setStyle("-fx-font-size: 12px; -fx-text-fill: #8b949e;");
        return label;
    }

    private static String defaultString(String value, String fallback) {
        return value == null || value.isBlank() ? fallback : value;
    }

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
