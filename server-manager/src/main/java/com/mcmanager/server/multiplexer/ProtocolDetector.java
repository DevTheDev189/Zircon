package com.mcmanager.server.multiplexer;

import io.netty.buffer.ByteBuf;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelPipeline;
import io.netty.handler.codec.ByteToMessageDecoder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;

/**
 * Inspects the first bytes of an incoming connection on the public port and
 * decides where to proxy it:
 *
 * <ul>
 *   <li><b>HTTP</b> (GET/POST/HEAD/OPTIONS/PUT/PATCH/DELETE/TRACE) → Javalin web
 *       server on the internal web port (admin UI, BOM endpoint, mod downloads).</li>
 *   <li><b>Anything else</b> → the Minecraft server on the internal MC port.</li>
 * </ul>
 *
 * <p>Already-buffered bytes are handed to the {@link ProxyHandler} so no data is
 * lost during the switch.
 */
public class ProtocolDetector extends ByteToMessageDecoder {

    private static final Logger log = LoggerFactory.getLogger(ProtocolDetector.class);

    /** ASCII byte sequences of HTTP request methods, each terminated by a space. */
    private static final byte[][] HTTP_PREFIXES = {
            {'G', 'E', 'T', ' '},
            {'P', 'O', 'S', 'T'},
            {'H', 'E', 'A', 'D'},
            {'O', 'P', 'T', 'I'},
            {'P', 'U', 'T', ' '},
            {'P', 'A', 'T', 'C'},
            {'D', 'E', 'L', 'E'},
            {'T', 'R', 'A', 'C'},
            {'C', 'O', 'N', 'N'},
    };

    private final String webHost;
    private final int webPort;
    private final String mcHost;
    private final int mcPort;

    public ProtocolDetector(String webHost, int webPort, String mcHost, int mcPort) {
        this.webHost = webHost;
        this.webPort = webPort;
        this.mcHost = mcHost;
        this.mcPort = mcPort;
    }

    @Override
    protected void decode(ChannelHandlerContext ctx, ByteBuf in, List<Object> out) {
        // Need at least 4 bytes to distinguish HTTP method prefixes.
        if (in.readableBytes() < 4) {
            return;
        }

        boolean http = isHttpMethod(in);
        String targetHost = http ? webHost : mcHost;
        int targetPort = http ? webPort : mcPort;

        // Keep any buffered bytes so they can be forwarded once the outbound leg connects.
        ByteBuf initialData = in.readRetainedSlice(in.readableBytes());

        ChannelPipeline pipeline = ctx.pipeline();
        pipeline.remove(this);
        pipeline.addLast(new ProxyHandler(targetHost, targetPort, initialData));

        log.debug("Proxying connection from {} to {}:{} ({})",
                ctx.channel().remoteAddress(), targetHost, targetPort, http ? "HTTP" : "Minecraft");
    }

    private boolean isHttpMethod(ByteBuf in) {
        for (byte[] prefix : HTTP_PREFIXES) {
            if (matchesPrefix(in, prefix)) {
                return true;
            }
        }
        return false;
    }

    private boolean matchesPrefix(ByteBuf in, byte[] prefix) {
        for (int i = 0; i < prefix.length; i++) {
            if (in.getByte(i) != prefix[i]) {
                return false;
            }
        }
        return true;
    }
}
