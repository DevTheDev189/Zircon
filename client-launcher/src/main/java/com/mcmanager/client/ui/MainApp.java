package com.mcmanager.client.ui;

import atlantafx.base.theme.PrimerDark;
import com.mcmanager.client.ui.controller.MainController;
import javafx.application.Application;
import javafx.application.Platform;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Scene;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.control.TextField;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;
import javafx.scene.shape.Circle;
import javafx.scene.text.Font;
import javafx.stage.Stage;

/**
 * JavaFX application shell for the launcher: AtlantaFX PrimerDark theme, clean
 * 900×550 layout, and the play/sign-in state machine (plan tasks 3.1 / 3.5).
 */
public class MainApp extends Application {

    @Override
    public void start(Stage stage) {
        Application.setUserAgentStylesheet(new PrimerDark().getUserAgentStylesheet());

        // --- Top bar ---
        Label logo = new Label("⚡");
        logo.setFont(new Font(20));
        logo.setStyle("-fx-background-color: #2da44e; -fx-text-fill: white;"
                + "-fx-background-radius: 8; -fx-padding: 6 10;");

        Label appName = new Label("McManager");
        appName.setStyle("-fx-font-size: 16px; -fx-font-weight: bold;");
        Label appSubtitle = new Label("mod-synced Minecraft launcher");
        appSubtitle.setStyle("-fx-font-size: 11px; -fx-text-fill: #8b949e;");
        VBox titleBox = new VBox(0, appName, appSubtitle);
        titleBox.setAlignment(Pos.CENTER_LEFT);

        HBox topBar = new HBox(10, logo, titleBox);
        topBar.setAlignment(Pos.CENTER_LEFT);

        // Right side: avatar + username + logout
        Circle avatar = new Circle(14, javafx.scene.paint.Color.web("#2da44e"));
        Label userLabel = new Label("Not signed in");
        userLabel.setStyle("-fx-font-size: 13px;");
        Button logoutButton = new Button("Logout");
        logoutButton.setVisible(false);

        HBox userBox = new HBox(8, avatar, userLabel, logoutButton);
        userBox.setAlignment(Pos.CENTER_RIGHT);

        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);
        HBox header = new HBox(topBar, spacer, userBox);
        header.setAlignment(Pos.CENTER);
        header.setPadding(new Insets(14, 20, 10, 20));

        // --- Center panel ---
        Label serverLabel = new Label("Server address");
        serverLabel.setStyle("-fx-font-size: 11px; -fx-text-fill: #8b949e;");
        TextField serverField = new TextField("localhost:25565");
        serverField.setPromptText("mc.example.com:25565");
        serverField.setStyle("-fx-font-size: 20px; -fx-padding: 10 12;");
        VBox serverBox = new VBox(6, serverLabel, serverField);

        Label statusLabel = new Label("Ready.");
        statusLabel.setWrapText(true);
        statusLabel.setStyle("-fx-font-size: 13px; -fx-text-fill: #c9d1d9;");

        ProgressBar progressBar = new ProgressBar(0);
        progressBar.setMaxWidth(Double.MAX_VALUE);
        progressBar.setPrefHeight(6);
        progressBar.setVisible(false);

        VBox center = new VBox(24, serverBox, statusLabel, progressBar);
        center.setAlignment(Pos.CENTER);
        center.setPadding(new Insets(40, 60, 30, 60));
        VBox.setVgrow(center, Priority.ALWAYS);

        // --- Bottom action area ---
        Button actionButton = new Button("PLAY");
        actionButton.setMaxWidth(Double.MAX_VALUE);
        actionButton.setPrefHeight(52);
        actionButton.setStyle(
                "-fx-background-color: #2da44e; -fx-text-fill: white; -fx-font-size: 18px;"
                + "-fx-font-weight: bold; -fx-background-radius: 10;");
        HBox actionBox = new HBox(actionButton);
        actionBox.setPadding(new Insets(0, 60, 24, 60));
        actionBox.setAlignment(Pos.CENTER);

        VBox root = new VBox(header, center, actionBox);
        root.setStyle("-fx-background-color: #0d1117;");

        MainController controller = new MainController(
                serverField, statusLabel, progressBar, actionButton, userLabel, logoutButton);
        controller.init();

        Scene scene = new Scene(root, 900, 550);
        stage.setTitle("McManager Launcher");
        stage.setScene(scene);
        stage.setMinWidth(720);
        stage.setMinHeight(480);
        stage.show();

        stage.setOnCloseRequest(e -> {
            controller.shutdown();
            Platform.exit();
        });
    }
}
