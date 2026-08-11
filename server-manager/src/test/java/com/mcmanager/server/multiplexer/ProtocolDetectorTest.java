package com.mcmanager.server.multiplexer;

import com.mcmanager.server.auth.JoinTicketManager;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.process.ConsoleStreamHandler;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import io.netty.channel.embedded.EmbeddedChannel;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ProtocolDetectorTest {

    @TempDir
    Path tempDir;

    private ProtocolDetector newInstanceModeDetector() throws IOException {
        ServerInstanceManager manager = new ServerInstanceManager(tempDir, new ConsoleStreamHandler());
        return new ProtocolDetector("127.0.0.1", 25564, "127.0.0.1", 25566, manager);
    }

    // ------------------------------------------------------------------
    // Join gate
    // ------------------------------------------------------------------

    @Test
    void rejectsLoginWithoutTicket() throws IOException {
        EmbeddedChannel channel = new EmbeddedChannel(newInstanceModeDetector());
        channel.writeInbound(Unpooled.wrappedBuffer(concat(handshake(2), loginStart("Steve"))));
        channel.runPendingTasks();

        // A framed Disconnect (Login) packet was written and the socket closed.
        Object out = channel.outboundMessages().poll();
        assertNotNull(out, "expected a disconnect packet to be written");
        assertTrue(out instanceof ByteBuf);
        ByteBuf disconnect = (ByteBuf) out;
        assertTrue(disconnect.toString(StandardCharsets.UTF_8).contains("Zircon Client Required"));
        disconnect.release();

        assertFalse(channel.isActive());
        channel.finishAndReleaseAll();
    }

    @Test
    void acceptsLoginWithTicket() throws IOException {
        JoinTicketManager.registerTicket("Steve");
        EmbeddedChannel channel = new EmbeddedChannel(newInstanceModeDetector());
        channel.writeInbound(Unpooled.wrappedBuffer(concat(handshake(2), loginStart("Steve"))));
        channel.runPendingTasks();

        // Handed off to the proxy leg: decoder removed, no disconnect written.
        assertNull(channel.pipeline().get(ProtocolDetector.class));
        assertTrue(channel.outboundMessages().isEmpty());
        channel.finishAndReleaseAll();
    }

    @Test
    void ticketIsConsumedOnFirstConnection() throws IOException {
        JoinTicketManager.registerTicket("Steve");
        EmbeddedChannel first = new EmbeddedChannel(newInstanceModeDetector());
        first.writeInbound(Unpooled.wrappedBuffer(concat(handshake(2), loginStart("Steve"))));
        first.runPendingTasks();
        assertNull(first.pipeline().get(ProtocolDetector.class)); // accepted
        first.finishAndReleaseAll();

        // Second connection with the same identity: ticket already consumed.
        EmbeddedChannel second = new EmbeddedChannel(newInstanceModeDetector());
        second.writeInbound(Unpooled.wrappedBuffer(concat(handshake(2), loginStart("Steve"))));
        second.runPendingTasks();
        assertFalse(second.outboundMessages().isEmpty()); // rejected
        assertFalse(second.isActive());
        second.finishAndReleaseAll();
    }

    @Test
    void waitsForLoginStartBeforeDeciding() throws IOException {
        EmbeddedChannel channel = new EmbeddedChannel(newInstanceModeDetector());
        channel.writeInbound(Unpooled.wrappedBuffer(handshake(2)));
        channel.runPendingTasks();

        // Handshake only: still in the decoder, nothing written or closed.
        assertNotNull(channel.pipeline().get(ProtocolDetector.class));
        assertTrue(channel.outboundMessages().isEmpty());
        assertTrue(channel.isActive());
        channel.finishAndReleaseAll();
    }

    @Test
    void statusPingIsNotGated() throws IOException {
        EmbeddedChannel channel = new EmbeddedChannel(newInstanceModeDetector());
        channel.writeInbound(Unpooled.wrappedBuffer(handshake(1)));
        channel.runPendingTasks();

        // Next state 1 (status): proxied immediately, no ticket required.
        assertNull(channel.pipeline().get(ProtocolDetector.class));
        assertTrue(channel.outboundMessages().isEmpty());
        channel.finishAndReleaseAll();
    }

    @Test
    void legacyModeBypassesTicketGate() {
        EmbeddedChannel channel = new EmbeddedChannel(
                new ProtocolDetector("127.0.0.1", 25564, "127.0.0.1", 25566));
        channel.writeInbound(Unpooled.wrappedBuffer(concat(handshake(2), loginStart("Steve"))));
        channel.runPendingTasks();

        assertNull(channel.pipeline().get(ProtocolDetector.class));
        assertTrue(channel.outboundMessages().isEmpty());
        channel.finishAndReleaseAll();
    }

    // ------------------------------------------------------------------
    // Packet builders (mirrors of the real Minecraft framing)
    // ------------------------------------------------------------------

    /** Handshake packet with the given next state (1 = status, 2 = login). */
    private static ByteBuf handshake(int nextState) {
        ByteBuf body = Unpooled.buffer();
        writeVarInt(body, 0); // packet id
        writeVarInt(body, 763); // protocol version (1.20.1)
        byte[] addr = "localhost".getBytes(StandardCharsets.US_ASCII);
        writeVarInt(body, addr.length);
        body.writeBytes(addr);
        body.writeShort(25565); // port
        writeVarInt(body, nextState);
        return frame(body);
    }

    /** Login Start packet carrying the player name. */
    private static ByteBuf loginStart(String name) {
        ByteBuf body = Unpooled.buffer();
        writeVarInt(body, 0); // packet id
        byte[] n = name.getBytes(StandardCharsets.US_ASCII);
        writeVarInt(body, n.length);
        body.writeBytes(n);
        return frame(body);
    }

    private static ByteBuf frame(ByteBuf body) {
        ByteBuf frame = Unpooled.buffer();
        writeVarInt(frame, body.readableBytes());
        frame.writeBytes(body);
        body.release();
        return frame;
    }

    private static ByteBuf concat(ByteBuf... buffers) {
        ByteBuf out = Unpooled.buffer();
        for (ByteBuf b : buffers) {
            out.writeBytes(b);
            b.release();
        }
        return out;
    }

    private static void writeVarInt(ByteBuf buf, int value) {
        while ((value & 0xFFFFFF80) != 0) {
            buf.writeByte((value & 0x7F) | 0x80);
            value >>>= 7;
        }
        buf.writeByte(value & 0x7F);
    }
}
