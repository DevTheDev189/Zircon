package com.mcmanager.client.ui;

import atlantafx.base.theme.PrimerDark;
import com.mcmanager.client.ui.component.Player3DRenderer;
import com.mcmanager.client.ui.controller.MainController;
import javafx.application.Application;
import javafx.application.Platform;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Node;
import javafx.scene.Scene;
import javafx.scene.control.Button;
import javafx.scene.control.CheckBox;
import javafx.scene.control.ComboBox;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.control.ScrollPane;
import javafx.scene.control.Slider;
import javafx.scene.control.TextField;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.layout.BorderPane;
import javafx.scene.layout.GridPane;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.StackPane;
import javafx.scene.layout.TilePane;
import javafx.scene.layout.VBox;
import javafx.scene.paint.Color;
import javafx.scene.shape.Rectangle;
import javafx.stage.Stage;

/**
 * JavaFX application shell for the Zircon launcher. The window is a
 * {@link StackPane} holding a Microsoft login overlay above the main layout
 * (left navigation sidebar + central view switcher).
 *
 * <p>Views: Servers (with 3D player preview), Play Offline, Skins (3D preview +
 * gallery), Settings, and Shaders &amp; Packs.
 */
public class MainApp extends Application {

    private static final String BG = "#0d1117";
    private static final String CARD = "#161b22";
    private static final String BORDER = "#30363d";
    private static final String ACCENT = "#47d2c9";
    private static final String MUTED = "#8b949e";
    private static final String TEXT = "#c9d1d9";

    @Override
    public void start(Stage stage) {
        Application.setUserAgentStylesheet(new PrimerDark().getUserAgentStylesheet());

        // ------------------------------------------------------------------
        // Sidebar
        // ------------------------------------------------------------------
        ImageView brandLockup = new ImageView(new Image(MainApp.class.getResourceAsStream("/zircon-title.png")));
        brandLockup.setFitHeight(26);
        brandLockup.setPreserveRatio(true);
        brandLockup.setSmooth(true);

        HBox brandHeader = new HBox(brandLockup);
        brandHeader.setAlignment(Pos.CENTER_LEFT);
        brandHeader.setPadding(new Insets(16, 16, 20, 16));

        Button navServers = navButton("⚡  Servers");
        Button navOffline = navButton("🎮  Play Offline");
        Button navSkins = navButton("👕  Skins");
        Button navSettings = navButton("⚙️  Settings");

        VBox navBox = new VBox(6, navServers, navOffline, navSkins, navSettings);
        navBox.setPadding(new Insets(0, 12, 0, 12));

        Region sidebarSpacer = new Region();
        VBox.setVgrow(sidebarSpacer, Priority.ALWAYS);

        ImageView userAvatar = new ImageView();
        userAvatar.setFitWidth(32);
        userAvatar.setFitHeight(32);
        userAvatar.setPreserveRatio(true);
        userAvatar.setSmooth(false);

        Label userLabel = new Label("Not signed in");
        userLabel.setStyle("-fx-font-size: 12px; -fx-text-fill: white; -fx-font-weight: bold;");

        Region userSpacer = new Region();
        HBox.setHgrow(userSpacer, Priority.ALWAYS);

        Button logoutButton = new Button("Logout");
        logoutButton.setStyle("-fx-font-size: 10px; -fx-padding: 2 8;");
        logoutButton.setVisible(false);

        HBox userHeader = new HBox(8, userAvatar, userLabel, userSpacer, logoutButton);
        userHeader.setAlignment(Pos.CENTER_LEFT);

        VBox userCard = new VBox(10, userHeader);
        userCard.setStyle("-fx-background-color: " + CARD + "; -fx-background-radius: 10; -fx-padding: 12;");

        VBox sidebar = new VBox(brandHeader, navBox, sidebarSpacer, userCard);
        sidebar.setPrefWidth(230);
        sidebar.setMinWidth(230);
        sidebar.setPadding(new Insets(0, 0, 16, 0));
        sidebar.setStyle("-fx-background-color: " + BG + "; -fx-border-color: #21262d; -fx-border-width: 0 1 0 0;");

        // ------------------------------------------------------------------
        // View 1: Servers
        // ------------------------------------------------------------------
        // Turn the figure slightly toward the servers list on its left.
        Player3DRenderer serverRenderer = new Player3DRenderer(360, 440, -10.0);

        Label sectionYourServers = sectionLabel("Your Servers");
        Button addServerBtn = new Button("+ Add Server");
        addServerBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: " + ACCENT + "; -fx-font-size: 12px; -fx-font-weight: bold;");

        Region yourSpacer = new Region();
        HBox.setHgrow(yourSpacer, Priority.ALWAYS);
        HBox yourHeader = new HBox(sectionYourServers, yourSpacer, addServerBtn);
        yourHeader.setAlignment(Pos.CENTER_LEFT);

        VBox savedServersContainer = new VBox(10);
        ScrollPane savedScroll = scrollPane(savedServersContainer, 210);

        Label sectionRecommended = sectionLabel("Recommended Servers");
        sectionRecommended.setStyle(sectionRecommended.getStyle() + " -fx-padding: 10 0 0 0;");

        VBox recommendedContainer = new VBox(10);
        ScrollPane recommendedScroll = scrollPane(recommendedContainer, 220);

        VBox serverListLeft = new VBox(12, yourHeader, savedScroll, sectionRecommended, recommendedScroll);

        StackPane serverPlayerBox = viewport(serverRenderer.getNode(), "3D Player Preview");
        HBox.setHgrow(serverPlayerBox, Priority.ALWAYS);

        HBox serverListView = new HBox(20, serverListLeft, serverPlayerBox);
        HBox.setHgrow(serverListLeft, Priority.ALWAYS);
        serverListView.setPadding(new Insets(20));

        // ------------------------------------------------------------------
        // View 2: Play Offline
        // ------------------------------------------------------------------
        Label offlineTitle = sectionLabel("Offline Instances");
        Button newInstanceBtn = new Button("+ New Instance");
        newInstanceBtn.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: #022c29; -fx-font-size: 12px; -fx-font-weight: bold;");

        Region offlineSpacer = new Region();
        HBox.setHgrow(offlineSpacer, Priority.ALWAYS);
        HBox offlineHeader = new HBox(offlineTitle, offlineSpacer, newInstanceBtn);
        offlineHeader.setAlignment(Pos.CENTER_LEFT);

        VBox offlineInstancesContainer = new VBox(10);
        ScrollPane offlineInstancesScroll = scrollPane(offlineInstancesContainer, 440);

        VBox offlineLeft = new VBox(12, offlineHeader, offlineInstancesScroll);
        offlineLeft.setPrefWidth(300);
        offlineLeft.setMinWidth(300);
        offlineLeft.setStyle("-fx-background-color: " + CARD + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-radius: 12; -fx-background-radius: 12; -fx-padding: 16;");

        // Right column: instance detail & options, grouped into clean sub-cards.
        Label offlineDetailTitle = sectionLabel("Select an instance");
        Label offlineVersionLabel = infoLabel("Minecraft: —");
        Label offlineLoaderLabel = infoLabel("Loader: —");
        Label offlineLoaderVersionLabel = infoLabel("Loader version: —");
        VBox offlineMetaCard = groupCard(offlineDetailTitle, offlineVersionLabel,
                offlineLoaderLabel, offlineLoaderVersionLabel);

        Label offlineModsHeader = sectionLabel("Mods");
        offlineModsHeader.setStyle(offlineModsHeader.getStyle() + " -fx-font-size: 13px;");

        VBox offlineModsContainer = new VBox(8);
        ScrollPane offlineModsScroll = scrollPane(offlineModsContainer, 110);

        Label offlineDropHint = new Label("Drop .jar mod files here (or click to browse)");
        offlineDropHint.setStyle("-fx-font-size: 12px; -fx-text-fill: " + MUTED + ";");
        VBox offlineDropZone = new VBox(offlineDropHint);
        offlineDropZone.setAlignment(Pos.CENTER);
        offlineDropZone.setPadding(new Insets(12));
        offlineDropZone.setStyle("-fx-background-color: " + BG + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-style: dashed; -fx-border-width: 1.5; -fx-border-radius: 8; -fx-background-radius: 8;");

        Label modrinthLabel = sectionLabel("Modrinth Search");
        modrinthLabel.setStyle(modrinthLabel.getStyle() + " -fx-font-size: 13px;");
        TextField modrinthQuery = new TextField();
        modrinthQuery.setPromptText("Search mods (e.g. Sodium)");
        Button modrinthSearchBtn = new Button("Search");
        modrinthSearchBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: " + ACCENT + "; -fx-font-size: 12px;");
        HBox modrinthRow = new HBox(8, modrinthQuery, modrinthSearchBtn);
        HBox.setHgrow(modrinthQuery, Priority.ALWAYS);

        VBox modrinthResultsContainer = new VBox(8);
        ScrollPane modrinthResultsScroll = scrollPane(modrinthResultsContainer, 110);

        VBox offlineModsCard = groupCard(offlineModsHeader, offlineDropZone, offlineModsScroll,
                modrinthLabel, modrinthRow, modrinthResultsScroll);

        // Shaders & texture packs for this instance: a per-instance local selection.
        Label offlinePacksHeader = sectionLabel("Shaders & Texture Packs");
        offlinePacksHeader.setStyle(offlinePacksHeader.getStyle() + " -fx-font-size: 13px;");

        // --- Shaders section ---
        Label offlineShadersSubHeader = sectionLabel("Shaders");
        offlineShadersSubHeader.setStyle(offlineShadersSubHeader.getStyle() + " -fx-font-size: 12px;");
        ComboBox<String> offlineShaderpackCombo = new ComboBox<>();
        offlineShaderpackCombo.setPrefWidth(220);
        offlineShaderpackCombo.setPromptText("Shaderpack (or None)");
        VBox offlineShaderpackList = new VBox(6);
        ScrollPane offlineShaderpackScroll = scrollPane(offlineShaderpackList, 90);
        Button offlineAddShaderpackBtn = new Button("+ Add Shaderpack (.zip)");
        offlineAddShaderpackBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: " + ACCENT + "; -fx-font-size: 11px; -fx-padding: 6 10;");
        TextField offlineShaderQuery = new TextField();
        offlineShaderQuery.setPromptText("Search Modrinth shaderpacks...");
        Button offlineShaderSearchBtn = new Button("Search");
        offlineShaderSearchBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: " + ACCENT + "; -fx-font-size: 12px;");
        HBox offlineShaderRow = new HBox(8, offlineShaderQuery, offlineShaderSearchBtn);
        HBox.setHgrow(offlineShaderQuery, Priority.ALWAYS);
        VBox offlineShaderResultsContainer = new VBox(8);
        ScrollPane offlineShaderResultsScroll = scrollPane(offlineShaderResultsContainer, 100);
        VBox offlineShadersSection = new VBox(6, offlineShadersSubHeader, offlineShaderpackCombo,
                offlineShaderpackScroll, offlineAddShaderpackBtn, offlineShaderRow, offlineShaderResultsScroll);

        // --- Texture Packs section ---
        Label offlineTextureSubHeader = sectionLabel("Texture Packs");
        offlineTextureSubHeader.setStyle(offlineTextureSubHeader.getStyle() + " -fx-font-size: 12px;");
        VBox offlineResourcepackContainer = new VBox(6);
        ScrollPane offlineResourcepackScroll = scrollPane(offlineResourcepackContainer, 90);
        Button offlineAddResourcepackBtn = new Button("+ Add Texture Pack (.zip)");
        offlineAddResourcepackBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: " + ACCENT + "; -fx-font-size: 11px; -fx-padding: 6 10;");
        TextField offlineTextureQuery = new TextField();
        offlineTextureQuery.setPromptText("Search Modrinth texture packs...");
        Button offlineTextureSearchBtn = new Button("Search");
        offlineTextureSearchBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: " + ACCENT + "; -fx-font-size: 12px;");
        HBox offlineTextureRow = new HBox(8, offlineTextureQuery, offlineTextureSearchBtn);
        HBox.setHgrow(offlineTextureQuery, Priority.ALWAYS);
        VBox offlineTextureResultsContainer = new VBox(8);
        ScrollPane offlineTextureResultsScroll = scrollPane(offlineTextureResultsContainer, 100);
        VBox offlineTextureSection = new VBox(6, offlineTextureSubHeader,
                offlineResourcepackScroll, offlineAddResourcepackBtn, offlineTextureRow, offlineTextureResultsScroll);

        VBox offlinePacksCard = groupCard(offlinePacksHeader, offlineShadersSection, offlineTextureSection);

        Button offlinePlayBtn = new Button("Play Offline");
        offlinePlayBtn.setMaxWidth(Double.MAX_VALUE);
        offlinePlayBtn.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: #022c29; -fx-font-weight: bold; -fx-padding: 10 16;");

        Button offlineDeleteBtn = new Button("Delete Instance");
        offlineDeleteBtn.setMaxWidth(Double.MAX_VALUE);
        offlineDeleteBtn.setStyle("-fx-background-color: #8b2b2b; -fx-text-fill: white; -fx-font-weight: bold; -fx-padding: 8 16;");

        VBox offlineDetail = new VBox(12, offlineMetaCard, offlineModsCard, offlinePacksCard);
        ScrollPane offlineDetailScroll = new ScrollPane(offlineDetail);
        offlineDetailScroll.setFitToWidth(true);
        offlineDetailScroll.setHbarPolicy(ScrollPane.ScrollBarPolicy.NEVER);
        offlineDetailScroll.setStyle("-fx-background-color: transparent; -fx-background: transparent;");
        VBox.setVgrow(offlineDetailScroll, Priority.ALWAYS);

        VBox offlineRight = new VBox(12, offlineDetailScroll, offlinePlayBtn, offlineDeleteBtn);
        offlineRight.setStyle("-fx-background-color: " + CARD + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-radius: 12; -fx-background-radius: 12; -fx-padding: 16;");
        HBox.setHgrow(offlineRight, Priority.ALWAYS);

        HBox offlineView = new HBox(20, offlineLeft, offlineRight);
        offlineView.setPadding(new Insets(20));

        // ------------------------------------------------------------------
        // View 3: Skins
        // ------------------------------------------------------------------
        // Turn the figure slightly toward the skin gallery on its right.
        Player3DRenderer skinsRenderer = new Player3DRenderer(380, 460, 10.0);

        StackPane skinsPlayerBox = viewport(skinsRenderer.getNode(), "3D Player Preview");
        HBox.setHgrow(skinsPlayerBox, Priority.ALWAYS);

        Button saveSkinBtn = new Button("SAVE");
        saveSkinBtn.setMaxWidth(Double.MAX_VALUE);
        saveSkinBtn.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: #022c29; "
                + "-fx-font-weight: bold; -fx-font-size: 14px; -fx-padding: 10 20; "
                + "-fx-background-radius: 8; -fx-cursor: hand;");

        Button removeSkinBtn = new Button("Remove Skin");
        removeSkinBtn.setMaxWidth(Double.MAX_VALUE);
        removeSkinBtn.setStyle("-fx-background-color: #8b2b2b; -fx-text-fill: white; "
                + "-fx-font-weight: bold; -fx-font-size: 14px; -fx-padding: 10 20; "
                + "-fx-background-radius: 8; -fx-cursor: hand;");

        Label skinStatus = new Label("Preview a skin, then press SAVE to activate it.");
        skinStatus.setStyle("-fx-font-size: 12px; -fx-text-fill: " + MUTED + ";");
        // Pin the label so the current skin's file name can never grow/shrink the
        // left card: a fixed width contribution, a fill-width stretch with wrap,
        // and a fixed two-line height.
        skinStatus.setWrapText(true);
        skinStatus.setPrefWidth(300);
        skinStatus.setMaxWidth(Double.MAX_VALUE);
        skinStatus.setPrefHeight(34);
        skinStatus.setMinHeight(34);
        skinStatus.setMaxHeight(34);

        VBox skinsLeft = new VBox(12, skinsPlayerBox, saveSkinBtn, removeSkinBtn, skinStatus);
        skinsLeft.setStyle("-fx-background-color: " + CARD + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-radius: 12; -fx-background-radius: 12; -fx-padding: 14;");
        HBox.setHgrow(skinsLeft, Priority.ALWAYS);

        TilePane skinsGallery = new TilePane();
        skinsGallery.setHgap(12);
        skinsGallery.setVgap(12);
        skinsGallery.setPadding(new Insets(4));

        ScrollPane skinsGalleryScroll = new ScrollPane(skinsGallery);
        skinsGalleryScroll.setFitToWidth(true);
        skinsGalleryScroll.setHbarPolicy(ScrollPane.ScrollBarPolicy.NEVER);
        skinsGalleryScroll.setStyle("-fx-background-color: transparent; -fx-background: transparent;");
        VBox.setVgrow(skinsGalleryScroll, Priority.ALWAYS);

        Label galleryHeader = sectionLabel("Skin Gallery");
        VBox skinsRight = new VBox(10, galleryHeader, skinsGalleryScroll);
        skinsRight.setStyle("-fx-background-color: " + CARD + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-radius: 12; -fx-background-radius: 12; -fx-padding: 14;");
        skinsRight.setPrefWidth(460);
        skinsRight.setMinWidth(300);
        HBox.setHgrow(skinsRight, Priority.ALWAYS);

        HBox skinsView = new HBox(20, skinsLeft, skinsRight);
        skinsView.setPadding(new Insets(20));

        // ------------------------------------------------------------------
        // View 4: Settings
        // ------------------------------------------------------------------
        Label settingsTitle = sectionLabel("Launcher Settings");

        Label ramLabel = infoLabel("Max Memory Allocation (RAM): 4 GB");
        Slider ramSlider = new Slider(2, 16, 4);
        ramSlider.setMajorTickUnit(2);
        ramSlider.setMinorTickCount(1);
        ramSlider.setSnapToTicks(true);
        ramSlider.setShowTickLabels(true);
        ramSlider.setPrefWidth(380);

        CheckBox strictVerifyCheck = new CheckBox("Strict Hash Verification (Abort on unverified mods)");
        strictVerifyCheck.setSelected(true);
        strictVerifyCheck.setStyle("-fx-text-fill: " + TEXT + ";");

        CheckBox trustDirectCheck = new CheckBox("Trust Direct Custom Mods");
        trustDirectCheck.setSelected(false);
        trustDirectCheck.setStyle("-fx-text-fill: " + TEXT + ";");

        VBox settingsView = new VBox(18, settingsTitle, ramLabel, ramSlider, strictVerifyCheck,
                trustDirectCheck);
        settingsView.setPadding(new Insets(24));
        settingsView.setMaxWidth(520);

        // ------------------------------------------------------------------
        // Center view switcher + status bar
        // ------------------------------------------------------------------
        StackPane centerContainer = new StackPane(serverListView, offlineView, skinsView, settingsView);
        offlineView.setVisible(false);
        skinsView.setVisible(false);
        settingsView.setVisible(false);

        Label statusLabel = new Label("Ready to play.");
        statusLabel.setStyle("-fx-font-size: 12px; -fx-text-fill: " + MUTED + ";");

        ProgressBar progressBar = new ProgressBar(0);
        progressBar.setMaxWidth(Double.MAX_VALUE);
        progressBar.setPrefHeight(6);
        progressBar.setVisible(false);

        VBox bottomStatusBox = new VBox(6, statusLabel, progressBar);
        bottomStatusBox.setPadding(new Insets(10, 20, 14, 20));
        bottomStatusBox.setStyle("-fx-background-color: " + BG + "; -fx-border-color: #21262d; -fx-border-width: 1 0 0 0;");

        BorderPane mainContentLayout = new BorderPane();
        mainContentLayout.setCenter(centerContainer);
        mainContentLayout.setBottom(bottomStatusBox);
        mainContentLayout.setStyle("-fx-background-color: " + CARD + ";");

        HBox mainLayout = new HBox(sidebar, mainContentLayout);
        HBox.setHgrow(mainContentLayout, Priority.ALWAYS);

        // ------------------------------------------------------------------
        // Login overlay
        // ------------------------------------------------------------------
        Label loginTitle = new Label("Welcome to Zircon");
        loginTitle.setStyle("-fx-font-size: 24px; -fx-font-weight: bold; -fx-text-fill: white;");

        Label loginSubtitle = new Label("Sign in with Microsoft to sync mods and play.");
        loginSubtitle.setStyle("-fx-font-size: 13px; -fx-text-fill: " + MUTED + ";");

        // Microsoft-branded sign-in button: "Login with" + a white pill holding
        // the four-square logo and the Microsoft wordmark (official button style).
        Label loginPrefix = new Label("Login with");
        loginPrefix.setStyle("-fx-text-fill: white; -fx-font-weight: bold; -fx-font-size: 15px;");

        Label microsoftWordmark = new Label("Microsoft");
        microsoftWordmark.setStyle("-fx-text-fill: #5e5e5e; -fx-font-weight: bold; -fx-font-size: 15px;");
        HBox microsoftPill = new HBox(7, microsoftLogo(16), microsoftWordmark);
        microsoftPill.setAlignment(Pos.CENTER);
        microsoftPill.setPadding(new Insets(5, 16, 5, 16));
        // A large radius turns the pill into a fully rounded capsule.
        microsoftPill.setStyle("-fx-background-color: white; -fx-background-radius: 999;");

        HBox loginButtonContent = new HBox(10, loginPrefix, microsoftPill);
        loginButtonContent.setAlignment(Pos.CENTER);

        Button loginButton = new Button();
        loginButton.setGraphic(loginButtonContent);
        loginButton.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: #022c29; -fx-font-weight: bold; "
                + "-fx-font-size: 15px; -fx-padding: 10 10 10 20; -fx-background-radius: 999;");

        Label loginStatus = new Label("");
        loginStatus.setStyle("-fx-font-size: 12px; -fx-text-fill: " + MUTED + ";");
        loginStatus.setWrapText(true);

        ImageView loginBrand = new ImageView(new Image(MainApp.class.getResourceAsStream("/zircon-title.png")));
        loginBrand.setFitHeight(38);
        loginBrand.setPreserveRatio(true);
        loginBrand.setMouseTransparent(true);

        VBox loginCard = new VBox(14, loginBrand, loginTitle, loginSubtitle, loginButton, loginStatus);
        loginCard.setAlignment(Pos.CENTER);
        loginCard.setMaxWidth(400);
        // Cap the height so the overlay's StackPane doesn't stretch the card to
        // fill the whole window — it should hug its content instead.
        loginCard.setMaxHeight(Region.USE_PREF_SIZE);
        loginCard.setPadding(new Insets(30));
        loginCard.setStyle("-fx-background-color: " + CARD + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-radius: 14; -fx-background-radius: 14;");

        StackPane loginView = new StackPane(loginCard);
        loginView.setStyle("-fx-background-color: " + BG + ";");

        StackPane root = new StackPane(mainLayout, loginView);
        // Cascade the brand accent to progress bars, sliders and checkboxes.
        root.setStyle("-fx-accent: " + ACCENT + ";");

        // ------------------------------------------------------------------
        // Controller wiring
        // ------------------------------------------------------------------
        MainController controller = new MainController(
                loginButton, loginStatus, loginView, mainLayout,
                userLabel, userAvatar, logoutButton, statusLabel, progressBar, stage,
                navServers, navOffline, navSkins, navSettings,
                serverListView, offlineView, skinsView, settingsView,
                savedServersContainer, recommendedContainer, addServerBtn,
                serverRenderer, skinsRenderer,
                saveSkinBtn, removeSkinBtn, skinStatus, skinsGallery,
                offlineInstancesContainer, newInstanceBtn,
                offlineDetailTitle, offlineVersionLabel, offlineLoaderLabel, offlineLoaderVersionLabel,
                offlineModsContainer, offlineDropZone, modrinthQuery, modrinthSearchBtn,
                modrinthResultsContainer,
                offlinePlayBtn, offlineDeleteBtn,
                offlineShaderpackCombo, offlineShaderpackList, offlineResourcepackContainer,
                offlineAddShaderpackBtn, offlineAddResourcepackBtn,
                offlineShaderQuery, offlineShaderSearchBtn, offlineShaderResultsContainer,
                offlineTextureQuery, offlineTextureSearchBtn, offlineTextureResultsContainer,
                ramSlider, ramLabel, strictVerifyCheck, trustDirectCheck);
        controller.init();

        Scene scene = new Scene(root, 1160, 720);
        stage.setTitle("Zircon Launcher");
        stage.setScene(scene);
        stage.getIcons().addAll(
                new Image(MainApp.class.getResourceAsStream("/zircon-icon-16.png")),
                new Image(MainApp.class.getResourceAsStream("/zircon-icon-32.png")),
                new Image(MainApp.class.getResourceAsStream("/zircon-icon-48.png")),
                new Image(MainApp.class.getResourceAsStream("/zircon-icon-64.png")),
                new Image(MainApp.class.getResourceAsStream("/zircon-icon-128.png")),
                new Image(MainApp.class.getResourceAsStream("/zircon-icon-256.png")));
        stage.setMinWidth(900);
        stage.setMinHeight(560);
        stage.show();

        stage.setOnCloseRequest(e -> {
            controller.shutdown();
            Platform.exit();
        });
    }

    // ------------------------------------------------------------------
    // Small styling helpers
    // ------------------------------------------------------------------

    private static Button navButton(String text) {
        Button btn = new Button(text);
        btn.setMaxWidth(Double.MAX_VALUE);
        btn.setAlignment(Pos.CENTER_LEFT);
        btn.setStyle("-fx-font-size: 14px; -fx-padding: 10 14; -fx-background-radius: 8; "
                + "-fx-background-color: transparent; -fx-text-fill: " + TEXT + ";");
        return btn;
    }

    private static Label sectionLabel(String text) {
        Label label = new Label(text);
        label.setStyle("-fx-font-size: 16px; -fx-font-weight: bold; -fx-text-fill: white;");
        return label;
    }

    private static Label infoLabel(String text) {
        Label label = new Label(text);
        label.setStyle("-fx-font-size: 12px; -fx-text-fill: " + TEXT + ";");
        return label;
    }

    private static ScrollPane scrollPane(Node content, double height) {
        ScrollPane scroll = new ScrollPane(content);
        scroll.setFitToWidth(true);
        scroll.setPrefHeight(height);
        scroll.setStyle("-fx-background-color: transparent; -fx-background: transparent;");
        return scroll;
    }

    private static StackPane viewport(Node content, String label) {
        Label hint = new Label(label);
        hint.setStyle("-fx-font-size: 11px; -fx-text-fill: " + MUTED + ";");
        StackPane box = new StackPane(content, hint);
        StackPane.setAlignment(hint, Pos.BOTTOM_CENTER);
        box.setMinWidth(300);
        box.setStyle("-fx-background-color: " + BG + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-radius: 12; -fx-background-radius: 12;");
        return box;
    }

    private static VBox groupCard(Node... children) {
        VBox card = new VBox(8, children);
        card.setPadding(new Insets(12));
        card.setStyle("-fx-background-color: " + BG + "; -fx-border-color: #21262d; "
                + "-fx-border-radius: 10; -fx-background-radius: 10;");
        return card;
    }

    /**
     * The four-square Microsoft logo mark (red/green/blue/yellow) rendered as
     * JavaFX shapes — the equivalent of the official SVG without SVG support.
     */
    private static Node microsoftLogo(double size) {
        String[] brandColors = {"#F25022", "#7FBA00", "#00A4EF", "#FFB900"};
        GridPane grid = new GridPane();
        grid.setHgap(1.5);
        grid.setVgap(1.5);
        for (int i = 0; i < 4; i++) {
            Rectangle tile = new Rectangle(size, size);
            tile.setArcWidth(size * 0.15);
            tile.setArcHeight(size * 0.15);
            tile.setFill(Color.web(brandColors[i]));
            grid.add(tile, i % 2, i / 2);
        }
        return grid;
    }
}
