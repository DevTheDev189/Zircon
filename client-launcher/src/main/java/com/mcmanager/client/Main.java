package com.mcmanager.client;

import com.mcmanager.client.ui.MainApp;

import java.util.ArrayList;
import java.util.List;

/**
 * Entry point of the launcher. JavaFX {@link Application} startup is delegated
 * to {@link MainApp}.
 *
 * <p>Recognized application arguments (safe to pass after {@code -jar}, unlike
 * JVM {@code -D} flags which shells can mangle):
 * <ul>
 *   <li>{@code --clientId=<AZURE_CLIENT_ID>} — Microsoft app registration id</li>
 *   <li>{@code --server=<host:port>} — prefill the server address field</li>
 * </ul>
 */
public final class Main {

    private Main() {
    }

    public static void main(String[] args) {
        List<String> passthrough = new ArrayList<>();
        for (String arg : args) {
            if (arg.startsWith("--clientId=")) {
                System.setProperty("mcmanager.clientId", arg.substring("--clientId=".length()));
            } else if (arg.startsWith("--server=")) {
                System.setProperty("mcmanager.serverAddress", arg.substring("--server=".length()));
            } else {
                passthrough.add(arg);
            }
        }
        MainApp.launch(MainApp.class, passthrough.toArray(String[]::new));
    }
}
