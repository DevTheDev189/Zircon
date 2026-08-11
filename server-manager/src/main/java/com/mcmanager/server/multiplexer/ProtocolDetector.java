package com.mcmanager.server.multiplexer;

import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.auth.JoinTicketManager;
import com.mcmanager.server.instance.ServerInstanceManager;
import io.netty.buffer.ByteBuf;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelPipeline;
import io.netty.handler.codec.ByteToMessageDecoder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.Locale;

/**
 * Inspects the first bytes of an incoming connection on the public port and
 * decides where to proxy it:
 *
 * <ul>
 *   <li><b>HTTP</b> (GET/POST/HEAD/PUT/DELETE/OPTIONS, each with a trailing
 *       space to avoid false positives on the Minecraft protocol) → Javalin web
 *       server on the internal web port (admin UI, mod downloads).</li>
 *   <li><b>Minecraft handshake</b> → the internal MC port of the instance whose
 *       id/name matches the handshake hostname (multi-instance), or the legacy
 *       default MC port when no instance manager is wired.</li>
 * </ul>
 *
 * <p>In instance mode, login-state connections must present a one-time join
 * ticket registered by the Zircon launcher (see {@link JoinTicketManager});
 * without one the socket is disconnected before reaching the game server.
 * Server-list status pings and legacy single-server mode are not gated.
 *
 * <p>All prefix matching is done <em>relative to the current reader index</em>
 * (never absolute index 0) so buffered bytes from previous reads don't skew the
 * detection. Already-buffered bytes are handed to the {@link ProxyHandler} so no
 * data is lost during the switch.
 */
public class ProtocolDetector extends ByteToMessageDecoder {

    private static final Logger log = LoggerFactory.getLogger(ProtocolDetector.class);

    /** Require trailing space so e.g. "GET " never collides with MC binary packets. */
    private static final byte[][] HTTP_PREFIXES = {
            {'G', 'E', 'T', ' '},
            {'P', 'O', 'S', 'T', ' '},
            {'H', 'E', 'A', 'D', ' '},
            {'P', 'U', 'T', ' '},
            {'D', 'E', 'L', 'E', 'T', 'E', ' '},
            {'O', 'P', 'T', 'I', 'O', 'N', 'S', ' '}
    };

    private final String webHost;
    private final int webPort;
    private final String mcHost;
    private final int mcPort;
    private final ServerInstanceManager instanceManager; // nullable → legacy single-server routing

    public ProtocolDetector(String webHost, int webPort, String mcHost, int mcPort) {
        this(webHost, webPort, mcHost, mcPort, null);
    }

    public ProtocolDetector(String webHost, int webPort, String mcHost, int mcPort,
                            ServerInstanceManager instanceManager) {
        this.webHost = webHost;
        this.webPort = webPort;
        this.mcHost = mcHost;
        this.mcPort = mcPort;
        this.instanceManager = instanceManager;
    }

    @Override
    protected void decode(ChannelHandlerContext ctx, ByteBuf in, List<Object> out) {
        // Need at least 5 bytes to reliably match an HTTP method.
        if (in.readableBytes() < 5) {
            return;
        }

        if (isHttpMethod(in)) {
            handoff(ctx, in, webHost, webPort, "HTTP");
            return;
        }

        int targetPort = mcPort;
        String kind = "Minecraft";
        if (instanceManager != null) {
            HandshakeResult handshake = tryParseHandshakeHostname(in);
            if (handshake == HandshakeResult.INCOMPLETE) {
                return; // wait for more bytes before deciding
            }
            if (handshake != HandshakeResult.NOT_A_HANDSHAKE) {
                InstanceConfig cfg = instanceManager.findByHostname(handshake.hostname);
                if (cfg != null) {
                    targetPort = cfg.getInternalMcPort();
                    kind = "Minecraft->" + cfg.getName();
                }

                // Zircon join gate (AGENT_PLAN_7): login connections must present a
                // one-time ticket registered by the launcher right before launch.
                // Status pings (next state 1) are always proxied; only login state
                // (next state 2) connections are gated.
                if (handshake.nextState == 2) {
                    LoginStartResult loginStart = tryParseLoginStart(in);
                    if (loginStart == LoginStartResult.INCOMPLETE) {
                        return; // Login Start packet not fully buffered yet
                    }
                    if (loginStart.username != null
                            && !JoinTicketManager.consumeTicket(loginStart.username)) {
                        log.info("Rejected connection for '{}' — no active Zircon join ticket",
                                loginStart.username);
                        ctx.writeAndFlush(MinecraftDisconnectUtil.createDisconnectPacket(
                                        MinecraftDisconnectUtil.buildCustomErrorMessage()))
                                .addListener(future -> ctx.close());
                        return;
                    }
                }
            }
        }
        handoff(ctx, in, mcHost, targetPort, kind);
    }

    // ------------------------------------------------------------------
    // HTTP detection
    // ------------------------------------------------------------------

    private boolean isHttpMethod(ByteBuf in) {
        int readerIndex = in.readerIndex(); // match relative to readerIndex!
        for (byte[] prefix : HTTP_PREFIXES) {
            if (matches(in, readerIndex, prefix)) {
                return true;
            }
        }
        return false;
    }

    private boolean matches(ByteBuf in, int offset, byte[] prefix) {
        if (in.readableBytes() < prefix.length) {
            return false;
        }
        for (int i = 0; i < prefix.length; i++) {
            if (in.getByte(offset + i) != prefix[i]) {
                return false;
            }
        }
        return true;
    }

    // ------------------------------------------------------------------
    // Minecraft handshake hostname parsing (for multi-instance routing)
    // ------------------------------------------------------------------

    private static final class HandshakeResult {
        static final HandshakeResult INCOMPLETE = new HandshakeResult(null, -1);
        static final HandshakeResult NOT_A_HANDSHAKE = new HandshakeResult("", -1);

        final String hostname;
        /** Next protocol state: 1 = status ping, 2 = login, -1 = unknown. */
        final int nextState;

        private HandshakeResult(String hostname, int nextState) {
            this.hostname = hostname;
            this.nextState = nextState;
        }
    }

    /**
     * Parses the server-list-ping handshake packet:
     * {@code [VarInt length][VarInt 0x00][VarInt protocol][VarInt addrLen][addr bytes][u16 port][VarInt nextState]}.
     *
     * @return {@link HandshakeResult#INCOMPLETE} when more bytes are needed,
     *         {@link HandshakeResult#NOT_A_HANDSHAKE} when the bytes can't be a
     *         handshake, or a result carrying the hostname and next state.
     */
    private static HandshakeResult tryParseHandshakeHostname(ByteBuf in) {
        int offset = in.readerIndex();
        int limit = in.writerIndex();

        VarInt length = readVarInt(in, offset, limit);
        if (length == null) return HandshakeResult.INCOMPLETE;
        offset += length.bytes;

        VarInt packetId = readVarInt(in, offset, limit);
        if (packetId == null) return HandshakeResult.INCOMPLETE;
        if (packetId.value != 0) return HandshakeResult.NOT_A_HANDSHAKE;
        offset += packetId.bytes;

        VarInt protocol = readVarInt(in, offset, limit);
        if (protocol == null) return HandshakeResult.INCOMPLETE;
        offset += protocol.bytes;

        VarInt addrLen = readVarInt(in, offset, limit);
        if (addrLen == null) return HandshakeResult.INCOMPLETE;
        if (addrLen.value < 1 || addrLen.value > 255) return HandshakeResult.NOT_A_HANDSHAKE;
        offset += addrLen.bytes;

        if (limit < offset + addrLen.value) return HandshakeResult.INCOMPLETE;
        StringBuilder sb = new StringBuilder(addrLen.value);
        for (int i = 0; i < addrLen.value; i++) {
            sb.append((char) in.getByte(offset + i));
        }
        String hostname = sb.toString().trim().toLowerCase(Locale.ROOT);
        offset += addrLen.value;

        // u16 port
        if (limit < offset + 2) return HandshakeResult.INCOMPLETE;
        offset += 2;

        // next state: 1 = status, 2 = login
        VarInt nextState = readVarInt(in, offset, limit);
        if (nextState == null) return HandshakeResult.INCOMPLETE;

        return hostname.isEmpty()
                ? HandshakeResult.NOT_A_HANDSHAKE
                : new HandshakeResult(hostname, nextState.value);
    }

    /**
     * The Login Start packet that follows a login-state handshake, or a marker
     * describing why it cannot be parsed yet.
     */
    private static final class LoginStartResult {
        static final LoginStartResult INCOMPLETE = new LoginStartResult(null);
        static final LoginStartResult NOT_LOGIN_START = new LoginStartResult(null);

        final String username;

        private LoginStartResult(String username) {
            this.username = username;
        }

        static LoginStartResult withUsername(String username) {
            return new LoginStartResult(username);
        }
    }

    /**
     * Parses the Login Start packet that follows a login-state handshake:
     * {@code [VarInt len][VarInt 0x00][VarInt nameLen][name bytes]} (the optional
     * trailing UUID, 1.19+, is intentionally ignored).
     *
     * @return {@link LoginStartResult#INCOMPLETE} when more bytes are needed,
     *         {@link LoginStartResult#NOT_LOGIN_START} when the next packet is not
     *         a Login Start, or a result carrying the player name.
     */
    private static LoginStartResult tryParseLoginStart(ByteBuf in) {
        int offset = in.readerIndex();
        int limit = in.writerIndex();

        // Skip the handshake frame: [VarInt length][length bytes].
        VarInt handshakeLen = readVarInt(in, offset, limit);
        if (handshakeLen == null) return LoginStartResult.INCOMPLETE;
        offset += handshakeLen.bytes;
        if (handshakeLen.value < 0 || limit < offset + handshakeLen.value) {
            return LoginStartResult.INCOMPLETE;
        }
        offset += handshakeLen.value;

        // Login Start frame: [VarInt length][VarInt 0x00][VarInt nameLen][name bytes].
        VarInt packetLen = readVarInt(in, offset, limit);
        if (packetLen == null) return LoginStartResult.INCOMPLETE;
        offset += packetLen.bytes;
        if (packetLen.value < 0 || limit < offset + packetLen.value) {
            return LoginStartResult.INCOMPLETE;
        }
        int packetEnd = offset + packetLen.value;

        VarInt packetId = readVarInt(in, offset, packetEnd);
        if (packetId == null) return LoginStartResult.INCOMPLETE;
        offset += packetId.bytes;
        if (packetId.value != 0) return LoginStartResult.NOT_LOGIN_START;

        VarInt nameLen = readVarInt(in, offset, packetEnd);
        if (nameLen == null) return LoginStartResult.INCOMPLETE;
        offset += nameLen.bytes;
        if (nameLen.value < 1 || nameLen.value > 16 || packetEnd < offset + nameLen.value) {
            return LoginStartResult.INCOMPLETE;
        }

        StringBuilder sb = new StringBuilder(nameLen.value);
        for (int i = 0; i < nameLen.value; i++) {
            sb.append((char) in.getByte(offset + i));
        }
        String username = sb.toString().trim();
        return username.isEmpty()
                ? LoginStartResult.NOT_LOGIN_START
                : LoginStartResult.withUsername(username);
    }

    private static final class VarInt {
        final int value;
        final int bytes;

        VarInt(int value, int bytes) {
            this.value = value;
            this.bytes = bytes;
        }
    }

    /** @return the varint at {@code offset} or {@code null} if the buffer is too short. */
    private static VarInt readVarInt(ByteBuf in, int offset, int limit) {
        int value = 0;
        int bytes = 0;
        int b;
        do {
            if (offset + bytes >= limit) {
                return null;
            }
            b = in.getByte(offset + bytes) & 0xFF;
            value |= (b & 0x7F) << (7 * bytes);
            bytes++;
            if (bytes > 5) {
                return new VarInt(-1, bytes); // malformed varint → not a handshake
            }
        } while ((b & 0x80) != 0);
        return new VarInt(value, bytes);
    }

    // ------------------------------------------------------------------
    // handoff
    // ------------------------------------------------------------------

    private void handoff(ChannelHandlerContext ctx, ByteBuf in, String host, int port, String kind) {
        // Keep any buffered bytes so they can be forwarded once the outbound leg connects.
        ByteBuf initialData = in.readRetainedSlice(in.readableBytes());

        ChannelPipeline pipeline = ctx.pipeline();
        pipeline.remove(this);
        pipeline.addLast(new ProxyHandler(host, port, initialData));

        log.debug("Proxying connection from {} to {}:{} ({})",
                ctx.channel().remoteAddress(), host, port, kind);
    }
}
