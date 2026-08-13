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
import javafx.scene.image.ImageView;
import javafx.scene.layout.BorderPane;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.StackPane;
import javafx.scene.layout.TilePane;
import javafx.scene.layout.VBox;
import javafx.scene.text.Font;
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
    private static final String ACCENT = "#2da44e";
    private static final String MUTED = "#8b949e";
    private static final String TEXT = "#c9d1d9";

    @Override
    public void start(Stage stage) {
        Application.setUserAgentStylesheet(new PrimerDark().getUserAgentStylesheet());

        // ------------------------------------------------------------------
        // Sidebar
        // ------------------------------------------------------------------
        Label logo = new Label("⚡");
        logo.setFont(new Font(22));
        logo.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: white; "
                + "-fx-background-radius: 8; -fx-padding: 4 10;");

        Label appName = new Label("Zircon");
        appName.setStyle("-fx-font-size: 16px; -fx-font-weight: bold; -fx-text-fill: white;");
        Label appSubtitle = new Label("mod-synced launcher");
        appSubtitle.setStyle("-fx-font-size: 10px; -fx-text-fill: " + MUTED + ";");
        VBox titleBox = new VBox(2, appName, appSubtitle);

        HBox brandHeader = new HBox(10, logo, titleBox);
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
        Player3DRenderer serverRenderer = new Player3DRenderer(360, 440);

        Label sectionYourServers = sectionLabel("Your Servers");
        Button addServerBtn = new Button("+ Add Server");
        addServerBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-font-size: 12px; -fx-font-weight: bold;");

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
        Label offlineTitle = sectionLabel("Offline Worlds");
        Button newWorldBtn = new Button("+ New World");
        newWorldBtn.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: white; -fx-font-size: 12px; -fx-font-weight: bold;");

        Region offlineSpacer = new Region();
        HBox.setHgrow(offlineSpacer, Priority.ALWAYS);
        HBox offlineHeader = new HBox(offlineTitle, offlineSpacer, newWorldBtn);
        offlineHeader.setAlignment(Pos.CENTER_LEFT);

        VBox offlineInstancesContainer = new VBox(10);
        ScrollPane offlineInstancesScroll = scrollPane(offlineInstancesContainer, 440);

        VBox offlineLeft = new VBox(12, offlineHeader, offlineInstancesScroll);
        offlineLeft.setPrefWidth(300);
        offlineLeft.setMinWidth(300);
        offlineLeft.setStyle("-fx-background-color: " + CARD + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-radius: 12; -fx-background-radius: 12; -fx-padding: 16;");

        // Right column: world detail & options, grouped into clean sub-cards.
        Label offlineDetailTitle = sectionLabel("Select a world");
        Label offlineVersionLabel = infoLabel("Minecraft: —");
        Label offlineLoaderLabel = infoLabel("Loader: —");
        Label offlineLoaderVersionLabel = infoLabel("Loader version: —");
        VBox offlineMetaCard = groupCard(offlineDetailTitle, offlineVersionLabel,
                offlineLoaderLabel, offlineLoaderVersionLabel);

        Label gameSettingsHeader = sectionLabel("Game Settings");
        gameSettingsHeader.setStyle(gameSettingsHeader.getStyle() + " -fx-font-size: 13px;");
        ComboBox<String> offlineGameModeCombo = new ComboBox<>();
        offlineGameModeCombo.getItems().addAll("survival", "creative", "adventure", "spectator");
        offlineGameModeCombo.setValue("survival");
        offlineGameModeCombo.setPrefWidth(160);

        CheckBox offlineAllowCheatsCheck = new CheckBox("Allow cheats");
        offlineAllowCheatsCheck.setStyle("-fx-text-fill: " + TEXT + ";");

        HBox offlineOptionsRow = new HBox(14, offlineGameModeCombo, offlineAllowCheatsCheck);
        offlineOptionsRow.setAlignment(Pos.CENTER_LEFT);
        VBox offlineSettingsCard = groupCard(gameSettingsHeader, offlineOptionsRow);

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
        modrinthSearchBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-font-size: 12px;");
        HBox modrinthRow = new HBox(8, modrinthQuery, modrinthSearchBtn);
        HBox.setHgrow(modrinthQuery, Priority.ALWAYS);

        VBox modrinthResultsContainer = new VBox(8);
        ScrollPane modrinthResultsScroll = scrollPane(modrinthResultsContainer, 110);

        VBox offlineModsCard = groupCard(offlineModsHeader, offlineDropZone, offlineModsScroll,
                modrinthLabel, modrinthRow, modrinthResultsScroll);

        // Shaders & texture packs for this world: a per-instance local selection.
        Label offlinePacksHeader = sectionLabel("Shaders & Texture Packs");
        offlinePacksHeader.setStyle(offlinePacksHeader.getStyle() + " -fx-font-size: 13px;");
        ComboBox<String> offlineShaderpackCombo = new ComboBox<>();
        offlineShaderpackCombo.setPrefWidth(220);
        offlineShaderpackCombo.setPromptText("Shaderpack (or None)");
        VBox offlineResourcepackContainer = new VBox(6);
        Button offlineAddShaderpackBtn = new Button("+ Add Shaderpack (.zip)");
        offlineAddShaderpackBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-font-size: 11px; -fx-padding: 6 10;");
        Button offlineAddResourcepackBtn = new Button("+ Add Texture Pack (.zip)");
        offlineAddResourcepackBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-font-size: 11px; -fx-padding: 6 10;");
        HBox offlinePackButtons = new HBox(8, offlineAddShaderpackBtn, offlineAddResourcepackBtn);
        HBox.setHgrow(offlineAddShaderpackBtn, Priority.ALWAYS);
        HBox.setHgrow(offlineAddResourcepackBtn, Priority.ALWAYS);
        VBox offlinePacksCard = groupCard(offlinePacksHeader, offlineShaderpackCombo,
                offlineResourcepackContainer, offlinePackButtons);

        Button offlinePlayBtn = new Button("Play Offline");
        offlinePlayBtn.setMaxWidth(Double.MAX_VALUE);
        offlinePlayBtn.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: white; -fx-font-weight: bold; -fx-padding: 10 16;");

        Button offlineDeleteBtn = new Button("Delete World");
        offlineDeleteBtn.setMaxWidth(Double.MAX_VALUE);
        offlineDeleteBtn.setStyle("-fx-background-color: #8b2b2b; -fx-text-fill: white; -fx-font-weight: bold; -fx-padding: 8 16;");

        VBox offlineDetail = new VBox(12, offlineMetaCard, offlineSettingsCard, offlineModsCard, offlinePacksCard);
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
        Player3DRenderer skinsRenderer = new Player3DRenderer(380, 460);

        StackPane skinsPlayerBox = viewport(skinsRenderer.getNode(), "3D Player Preview");
        HBox.setHgrow(skinsPlayerBox, Priority.ALWAYS);

        Button saveSkinBtn = new Button("SAVE");
        saveSkinBtn.setMaxWidth(Double.MAX_VALUE);
        saveSkinBtn.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: white; "
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
        skinsRight.setMinWidth(380);

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

        Label clientIdLabel = infoLabel("Azure App Client ID");
        TextField clientIdField = new TextField();
        clientIdField.setPromptText("Microsoft App Client ID");
        clientIdField.setPrefWidth(380);

        VBox settingsView = new VBox(18, settingsTitle, ramLabel, ramSlider, strictVerifyCheck,
                trustDirectCheck, clientIdLabel, clientIdField);
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
        loginTitle.setStyle("-fx-font-size: 26px; -fx-font-weight: bold; -fx-text-fill: white;");

        Label loginSubtitle = new Label("Sign in with Microsoft to sync mods and play.");
        loginSubtitle.setStyle("-fx-font-size: 13px; -fx-text-fill: " + MUTED + ";");

        Button loginButton = new Button("Login with Microsoft");
        loginButton.setStyle("-fx-background-color: " + ACCENT + "; -fx-text-fill: white; -fx-font-weight: bold; "
                + "-fx-font-size: 15px; -fx-padding: 12 26; -fx-background-radius: 8;");

        Label loginStatus = new Label("");
        loginStatus.setStyle("-fx-font-size: 12px; -fx-text-fill: " + MUTED + ";");
        loginStatus.setWrapText(true);

        VBox loginCard = new VBox(14, loginTitle, loginSubtitle, loginButton, loginStatus);
        loginCard.setAlignment(Pos.CENTER);
        loginCard.setMaxWidth(420);
        loginCard.setPadding(new Insets(40));
        loginCard.setStyle("-fx-background-color: " + CARD + "; -fx-border-color: " + BORDER + "; "
                + "-fx-border-radius: 14; -fx-background-radius: 14;");

        StackPane loginView = new StackPane(loginCard);
        loginView.setStyle("-fx-background-color: " + BG + ";");

        StackPane root = new StackPane(mainLayout, loginView);

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
                offlineInstancesContainer, newWorldBtn,
                offlineDetailTitle, offlineVersionLabel, offlineLoaderLabel, offlineLoaderVersionLabel,
                offlineModsContainer, offlineDropZone, modrinthQuery, modrinthSearchBtn,
                modrinthResultsContainer, offlineGameModeCombo, offlineAllowCheatsCheck,
                offlinePlayBtn, offlineDeleteBtn,
                offlineShaderpackCombo, offlineResourcepackContainer,
                offlineAddShaderpackBtn, offlineAddResourcepackBtn,
                ramSlider, ramLabel, strictVerifyCheck, trustDirectCheck, clientIdField);
        controller.init();

        Scene scene = new Scene(root, 1160, 720);
        stage.setTitle("Zircon Launcher");
        stage.setScene(scene);
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
}
