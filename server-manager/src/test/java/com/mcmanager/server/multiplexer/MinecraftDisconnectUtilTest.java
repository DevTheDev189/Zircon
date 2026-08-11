package com.mcmanager.server.multiplexer;

import com.google.gson.JsonParser;
import io.netty.buffer.ByteBuf;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MinecraftDisconnectUtilTest {

    @Test
    void disconnectPacketIsCorrectlyFramed() {
        String message = MinecraftDisconnectUtil.buildCustomErrorMessage();
        ByteBuf frame = MinecraftDisconnectUtil.createDisconnectPacket(message);

        // [VarInt frameLen][VarInt 0x00][VarInt msgLen][msg bytes]
        VarIntResult frameLen = readVarInt(frame, 0, frame.readableBytes());
        assertEquals(frame.readableBytes(), frameLen.bytes + frameLen.value);

        VarIntResult packetId = readVarInt(frame, frameLen.bytes, frameLen.bytes + frameLen.value);
        assertEquals(0, packetId.value); // Login Disconnect packet id

        int packetStart = frameLen.bytes + packetId.bytes;
        VarIntResult msgLen = readVarInt(frame, packetStart, frameLen.bytes + frameLen.value);
        assertEquals(message.getBytes(StandardCharsets.UTF_8).length, msgLen.value);

        String payload = frame.toString(packetStart + msgLen.bytes,
                msgLen.value, StandardCharsets.UTF_8);
        assertEquals(message, payload);

        frame.release();
    }

    @Test
    void errorMessageIsValidJson() {
        String message = MinecraftDisconnectUtil.buildCustomErrorMessage();
        var json = JsonParser.parseString(message).getAsJsonObject();
        assertTrue(json.get("text").getAsString().contains("Zircon Client Required"));
    }

    private static final class VarIntResult {
        final int value;
        final int bytes;

        VarIntResult(int value, int bytes) {
            this.value = value;
            this.bytes = bytes;
        }
    }

    private static VarIntResult readVarInt(ByteBuf buf, int offset, int limit) {
        int value = 0;
        int bytes = 0;
        int b;
        do {
            b = buf.getByte(offset + bytes) & 0xFF;
            value |= (b & 0x7F) << (7 * bytes);
            bytes++;
        } while ((b & 0x80) != 0);
        return new VarIntResult(value, bytes);
    }
}
