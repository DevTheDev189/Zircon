package com.mcmanager.server.multiplexer;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;

import java.nio.charset.StandardCharsets;

/**
 * Builds Minecraft protocol frames, specifically the clientbound
 * {@code Disconnect (Login)} packet (packet ID {@code 0x00} in the login
 * state) that the connection gate writes before closing a rejected socket.
 */
public final class MinecraftDisconnectUtil {

    private MinecraftDisconnectUtil() {
    }

    /**
     * Creates a framed Minecraft {@code Disconnect (Login)} packet carrying a
     * chat JSON message.
     *
     * <p>Frame layout: {@code [VarInt frameLen][VarInt 0x00][VarInt msgLen][msg bytes]}.
     */
    public static ByteBuf createDisconnectPacket(String jsonMessage) {
        byte[] messageBytes = jsonMessage.getBytes(StandardCharsets.UTF_8);

        ByteBuf packetBuf = Unpooled.buffer();
        writeVarInt(packetBuf, 0x00); // Packet ID for Login Disconnect
        writeVarInt(packetBuf, messageBytes.length); // String length
        packetBuf.writeBytes(messageBytes); // String payload

        ByteBuf frameBuf = Unpooled.buffer();
        writeVarInt(frameBuf, packetBuf.readableBytes()); // Total frame length
        frameBuf.writeBytes(packetBuf);
        packetBuf.release();

        return frameBuf;
    }

    /** The in-game message shown when a connection is rejected by the join gate. */
    public static String buildCustomErrorMessage() {
        return """
                {
                  "text": "⚡ Zircon Client Required\\n\\n",
                  "color": "red",
                  "bold": true,
                  "extra": [
                    {
                      "text": "You must use the official Zircon Launcher to join this server.\\n\\n",
                      "color": "gray",
                      "bold": false
                    },
                    {
                      "text": "Launch the game using your Zircon client to auto-sync mods and connect.",
                      "color": "gold"
                    }
                  ]
                }
                """;
    }

    private static void writeVarInt(ByteBuf buf, int value) {
        while ((value & 0xFFFFFF80) != 0) {
            buf.writeByte((value & 0x7F) | 0x80);
            value >>>= 7;
        }
        buf.writeByte(value & 0x7F);
    }
}
