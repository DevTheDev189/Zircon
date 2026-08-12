package com.mcmanager.client.ui;

import atlantafx.base.theme.PrimerDark;
import com.mcmanager.client.model.SavedServer;
import com.mcmanager.client.ui.controller.MainController;
import javafx.application.Application;
import javafx.application.Platform;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Scene;
import javafx.scene.control.*;
import javafx.scene.image.ImageView;
import javafx.scene.layout.*;
import javafx.scene.shape.Circle;
import javafx.scene.text.Font;
import javafx.stage.Stage;

/**
 * JavaFX application shell for McManager client:
 * Left Navigation Sidebar (Server List, Change Skin, Settings, Play Offline)
 * and rich central views matching the required launcher layout.
 */
public class MainApp extends Application {

    @Override
    public void start(Stage stage) {
        Application.setUserAgentStylesheet(new PrimerDark().getUserAgentStylesheet());

        // --- Sidebar ---
        Label logo = new Label("⚡");
        logo.setFont(new Font(22));
        logo.setStyle("-fx-background-color: #2da44e; -fx-text-fill: white; "
                + "-fx-background-radius: 8; -fx-padding: 4 10;");

        Label appName = new Label("McManager");
        appName.setStyle("-fx-font-size: 16px; -fx-font-weight: bold; -fx-text-fill: white;");
        Label appSubtitle = new Label("mod-synced launcher");
        appSubtitle.setStyle("-fx-font-size: 10px; -fx-text-fill: #8b949e;");
        VBox titleBox = new VBox(2, appName, appSubtitle);

        HBox brandHeader = new HBox(10, logo, titleBox);
        brandHeader.setAlignment(Pos.CENTER_LEFT);
        brandHeader.setPadding(new Insets(16, 16, 20, 16));

        // Navigation Buttons
        Button navServerList = new Button("⚡  Server List");
        Button navChangeSkin = new Button("👕  Change Skin");
        Button navSettings = new Button("⚙️  Settings");
        Button navShadersPacks = new Button("🎨  Shaders & Packs");

        for (Button btn : new Button[]{navServerList, navChangeSkin, navSettings, navShadersPacks}) {
            btn.setMaxWidth(Double.MAX_VALUE);
            btn.setAlignment(Pos.CENTER_LEFT);
            btn.setStyle("-fx-font-size: 14px; -fx-padding: 10 14; -fx-background-radius: 8; "
                    + "-fx-background-color: transparent; -fx-text-fill: #c9d1d9;");
        }

        VBox navBox = new VBox(6, navServerList, navChangeSkin, navSettings, navShadersPacks);
        navBox.setPadding(new Insets(0, 12, 0, 12));

        Region sidebarSpacer = new Region();
        VBox.setVgrow(sidebarSpacer, Priority.ALWAYS);

        // Sidebar Footer User Card
        Circle avatar = new Circle(14, javafx.scene.paint.Color.web("#2da44e"));
        Label userLabel = new Label("Not signed in");
        userLabel.setStyle("-fx-font-size: 12px; -fx-text-fill: white; -fx-font-weight: bold;");
        Button logoutButton = new Button("Logout");
        logoutButton.setStyle("-fx-font-size: 10px; -fx-padding: 2 8;");
        logoutButton.setVisible(false);

        HBox userHeader = new HBox(8, avatar, userLabel, logoutButton);
        userHeader.setAlignment(Pos.CENTER_LEFT);

        VBox userCard = new VBox(10, userHeader);
        userCard.setStyle("-fx-background-color: #161b22; -fx-background-radius: 10; -fx-padding: 12;");

        VBox sidebar = new VBox(brandHeader, navBox, sidebarSpacer, userCard);
        sidebar.setPrefWidth(220);
        sidebar.setMinWidth(220);
        sidebar.setPadding(new Insets(0, 0, 16, 0));
        sidebar.setStyle("-fx-background-color: #0d1117; -fx-border-color: #21262d; -fx-border-width: 0 1 0 0;");

        // --- View 1: Server List ---
        Label sectionYourServers = new Label("Your Servers");
        sectionYourServers.setStyle("-fx-font-size: 16px; -fx-font-weight: bold; -fx-text-fill: white;");

        Button addServerBtn = new Button("+ Add Server");
        addServerBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-font-size: 12px; -fx-font-weight: bold;");

        Region yourSpacer = new Region();
        HBox.setHgrow(yourSpacer, Priority.ALWAYS);
        HBox yourHeader = new HBox(sectionYourServers, yourSpacer, addServerBtn);
        yourHeader.setAlignment(Pos.CENTER_LEFT);

        VBox savedServersContainer = new VBox(10);
        ScrollPane savedScroll = new ScrollPane(savedServersContainer);
        savedScroll.setFitToWidth(true);
        savedScroll.setPrefHeight(200);
        savedScroll.setStyle("-fx-background-color: transparent; -fx-background: transparent;");

        Label sectionRecommended = new Label("Recommended Servers");
        sectionRecommended.setStyle("-fx-font-size: 16px; -fx-font-weight: bold; -fx-text-fill: white; -fx-padding: 10 0 0 0;");

        VBox recommendedContainer = new VBox(10);

        VBox serverListView = new VBox(14, yourHeader, savedScroll, sectionRecommended, recommendedContainer);
        serverListView.setPadding(new Insets(20));

        // --- View 2: Change Skin ---
        Label skinTitle = new Label("Skin Customizer");
        skinTitle.setStyle("-fx-font-size: 18px; -fx-font-weight: bold; -fx-text-fill: white;");
        Label skinSubtitle = new Label("Upload a custom 64x64 PNG skin for your Minecraft player");
        skinSubtitle.setStyle("-fx-font-size: 12px; -fx-text-fill: #8b949e;");

        ImageView skinPreview = new ImageView();
        skinPreview.setFitWidth(128);
        skinPreview.setFitHeight(128);
        skinPreview.setPreserveRatio(true);
        skinPreview.setSmooth(false); // Sharp pixel scaling

        StackPane skinBox = new StackPane(skinPreview);
        skinBox.setPrefSize(160, 160);
        skinBox.setMaxSize(160, 160);
        skinBox.setStyle("-fx-background-color: #161b22; -fx-border-color: #30363d; -fx-border-radius: 12; -fx-background-radius: 12;");

        Button uploadSkinBtn = new Button("Upload .PNG Skin");
        uploadSkinBtn.setStyle("-fx-background-color: #2da44e; -fx-text-fill: white; -fx-font-weight: bold; -fx-padding: 8 16;");

        Button resetSkinBtn = new Button("Reset to Default");
        resetSkinBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #c9d1d9; -fx-padding: 8 16;");

        HBox skinActionBox = new HBox(12, uploadSkinBtn, resetSkinBtn);
        skinActionBox.setAlignment(Pos.CENTER);

        Label skinStatus = new Label("Default Steve / Alex");
        skinStatus.setStyle("-fx-font-size: 12px; -fx-text-fill: #8b949e;");

        VBox changeSkinView = new VBox(16, skinTitle, skinSubtitle, skinBox, skinActionBox, skinStatus);
        changeSkinView.setAlignment(Pos.TOP_CENTER);
        changeSkinView.setPadding(new Insets(30));

        // --- View 3: Settings ---
        Label settingsTitle = new Label("Launcher Settings");
        settingsTitle.setStyle("-fx-font-size: 18px; -fx-font-weight: bold; -fx-text-fill: white;");

        Label ramLabel = new Label("Max Memory Allocation (RAM): 4 GB");
        ramLabel.setStyle("-fx-font-size: 13px; -fx-text-fill: #c9d1d9;");
        Slider ramSlider = new Slider(2, 16, 4);
        ramSlider.setMajorTickUnit(2);
        ramSlider.setMinorTickCount(1);
        ramSlider.setSnapToTicks(true);
        ramSlider.setShowTickLabels(true);

        CheckBox strictVerifyCheck = new CheckBox("Strict Hash Verification (Abort on unverified mods)");
        strictVerifyCheck.setSelected(true);

        CheckBox trustDirectCheck = new CheckBox("Trust Direct Custom Mods");
        trustDirectCheck.setSelected(false);

        Label clientIdLabel = new Label("Azure App Client ID");
        clientIdLabel.setStyle("-fx-font-size: 12px; -fx-text-fill: #8b949e;");
        TextField clientIdField = new TextField();
        clientIdField.setPromptText("Microsoft App Client ID");

        VBox settingsView = new VBox(18, settingsTitle, ramLabel, ramSlider, strictVerifyCheck, trustDirectCheck, clientIdLabel, clientIdField);
        settingsView.setPadding(new Insets(24));
        settingsView.setMaxWidth(500);

        // --- View 4: Shaders & Texture Packs ---
        Label packsTitle = new Label("Shaders & Texture Packs");
        packsTitle.setStyle("-fx-font-size: 18px; -fx-font-weight: bold; -fx-text-fill: white;");

        ComboBox<SavedServer> packServerPicker = new ComboBox<>();
        packServerPicker.setPromptText("Choose a server");
        packServerPicker.setCellFactory(list -> new ListCell<>() {
            @Override
            protected void updateItem(SavedServer item, boolean empty) {
                super.updateItem(item, empty);
                setText(empty || item == null ? null : item.getName());
            }
        });
        packServerPicker.setButtonCell(packServerPicker.getCellFactory().call(null));
        packServerPicker.setPrefWidth(220);

        Button packSyncBtn = new Button("Sync Now");
        packSyncBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-font-size: 12px; -fx-padding: 6 14;");

        Label packStatusLabel = new Label("Choose a server to sync its packs.");
        packStatusLabel.setStyle("-fx-font-size: 11px; -fx-text-fill: #8b949e;");

        HBox packServerRow = new HBox(10, packServerPicker, packSyncBtn);
        packServerRow.setAlignment(Pos.CENTER_LEFT);

        VBox packHeader = new VBox(6, packsTitle, packServerRow, packStatusLabel);

        Label shaderpackHeader = new Label("Shader Packs (pick one, or None)");
        shaderpackHeader.setStyle("-fx-font-size: 13px; -fx-font-weight: bold; -fx-text-fill: white;");
        VBox shaderpackListContainer = new VBox(6);
        ScrollPane shaderpackScroll = new ScrollPane(shaderpackListContainer);
        shaderpackScroll.setFitToWidth(true);
        shaderpackScroll.setPrefHeight(180);
        shaderpackScroll.setStyle("-fx-background-color: transparent; -fx-background: transparent;");
        Button addShaderpackBtn = new Button("+ Add Local Shaderpack (.zip)");
        addShaderpackBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-font-size: 11px; -fx-padding: 6 10;");
        VBox shaderpackCard = new VBox(8, shaderpackHeader, shaderpackScroll, addShaderpackBtn);
        shaderpackCard.setPadding(new Insets(14));
        shaderpackCard.setStyle("-fx-background-color: #161b22; -fx-border-color: #30363d; -fx-border-radius: 10; -fx-background-radius: 10;");
        HBox.setHgrow(shaderpackCard, Priority.ALWAYS);

        Label resourcepackHeader = new Label("Texture Packs (check any you want)");
        resourcepackHeader.setStyle("-fx-font-size: 13px; -fx-font-weight: bold; -fx-text-fill: white;");
        VBox resourcepackListContainer = new VBox(6);
        ScrollPane resourcepackScroll = new ScrollPane(resourcepackListContainer);
        resourcepackScroll.setFitToWidth(true);
        resourcepackScroll.setPrefHeight(180);
        resourcepackScroll.setStyle("-fx-background-color: transparent; -fx-background: transparent;");
        Button addResourcepackBtn = new Button("+ Add Local Texture Pack (.zip)");
        addResourcepackBtn.setStyle("-fx-background-color: #21262d; -fx-text-fill: #58a6ff; -fx-font-size: 11px; -fx-padding: 6 10;");
        VBox resourcepackCard = new VBox(8, resourcepackHeader, resourcepackScroll, addResourcepackBtn);
        resourcepackCard.setPadding(new Insets(14));
        resourcepackCard.setStyle("-fx-background-color: #161b22; -fx-border-color: #30363d; -fx-border-radius: 10; -fx-background-radius: 10;");
        HBox.setHgrow(resourcepackCard, Priority.ALWAYS);

        HBox packCardsRow = new HBox(16, shaderpackCard, resourcepackCard);

        VBox shadersPacksView = new VBox(16, packHeader, packCardsRow);
        shadersPacksView.setPadding(new Insets(24));

        // --- Central View Switcher ---
        StackPane centerContainer = new StackPane(serverListView, changeSkinView, settingsView, shadersPacksView);
        changeSkinView.setVisible(false);
        settingsView.setVisible(false);
        shadersPacksView.setVisible(false);

        // --- Bottom Notification / Launch Bar ---
        Label statusLabel = new Label("Ready to play.");
        statusLabel.setStyle("-fx-font-size: 12px; -fx-text-fill: #8b949e;");

        ProgressBar progressBar = new ProgressBar(0);
        progressBar.setMaxWidth(Double.MAX_VALUE);
        progressBar.setPrefHeight(6);
        progressBar.setVisible(false);

        VBox bottomStatusBox = new VBox(6, statusLabel, progressBar);
        bottomStatusBox.setPadding(new Insets(10, 20, 14, 20));
        bottomStatusBox.setStyle("-fx-background-color: #0d1117; -fx-border-color: #21262d; -fx-border-width: 1 0 0 0;");

        BorderPane mainContentLayout = new BorderPane();
        mainContentLayout.setCenter(centerContainer);
        mainContentLayout.setBottom(bottomStatusBox);
        mainContentLayout.setStyle("-fx-background-color: #161b22;");

        HBox root = new HBox(sidebar, mainContentLayout);
        HBox.setHgrow(mainContentLayout, Priority.ALWAYS);

        MainController controller = new MainController(
                navServerList, navChangeSkin, navSettings,
                serverListView, changeSkinView, settingsView,
                savedServersContainer, recommendedContainer, addServerBtn,
                skinPreview, uploadSkinBtn, resetSkinBtn, skinStatus,
                ramSlider, ramLabel, strictVerifyCheck, trustDirectCheck, clientIdField,
                statusLabel, progressBar, userLabel, logoutButton, stage,
                navShadersPacks, shadersPacksView, packServerPicker, packSyncBtn, packStatusLabel,
                shaderpackListContainer, resourcepackListContainer, shaderpackCard, resourcepackCard,
                addShaderpackBtn, addResourcepackBtn
        );
        controller.init();

        Scene scene = new Scene(root, 960, 600);
        stage.setTitle("McManager Launcher");
        stage.setScene(scene);
        stage.setMinWidth(800);
        stage.setMinHeight(520);
        stage.show();

        stage.setOnCloseRequest(e -> {
            controller.shutdown();
            Platform.exit();
        });
    }
}
