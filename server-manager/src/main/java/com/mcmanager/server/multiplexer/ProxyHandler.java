package com.mcmanager.server.multiplexer;

import io.netty.bootstrap.Bootstrap;
import io.netty.buffer.ByteBuf;
import io.netty.channel.Channel;
import io.netty.channel.ChannelFuture;
import io.netty.channel.ChannelFutureListener;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.ChannelOption;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayDeque;
import java.util.Deque;

/**
 * A transparent bidirectional TCP proxy leg. Installed on the inbound (client)
 * channel; on connect it opens the outbound channel to the backend and pipes
 * traffic both ways.
 *
 * <p>Handles abrupt disconnects (e.g. {@code ECONNRESET} when a player closes
 * the game) by closing both channels without leaking, per plan task 4.2.
 */
public class ProxyHandler extends ChannelInboundHandlerAdapter {

    private static final Logger log = LoggerFactory.getLogger(ProxyHandler.class);

    private final String remoteHost;
    private final int remotePort;
    private ByteBuf initialData;

    private Channel inboundChannel;
    private Channel outboundChannel;
    private final Deque<ByteBuf> pending = new ArrayDeque<>();
    private boolean outboundActive = false;
    private boolean closing = false;

    public ProxyHandler(String remoteHost, int remotePort, ByteBuf initialData) {
        this.remoteHost = remoteHost;
        this.remotePort = remotePort;
        this.initialData = initialData;
    }

    @Override
    public void handlerAdded(ChannelHandlerContext ctx) {
        // When swapped in by the protocol detector the channel is already active,
        // so channelActive() will not fire; start the connection from here instead.
        if (ctx.channel().isActive()) {
            connectOutbound(ctx);
        }
    }

    @Override
    public void channelActive(ChannelHandlerContext ctx) {
        // Case: handler installed before the channel became active.
        connectOutbound(ctx);
    }

    private void connectOutbound(ChannelHandlerContext ctx) {
        inboundChannel = ctx.channel();

        Bootstrap bootstrap = new Bootstrap()
                .group(ctx.channel().eventLoop())
                .channel(ctx.channel().getClass())
                .option(ChannelOption.AUTO_READ, false)
                .handler(new BackendHandler());

        ChannelFuture connectFuture = bootstrap.connect(remoteHost, remotePort);
        outboundChannel = connectFuture.channel();
        connectFuture.addListener((ChannelFutureListener) future -> {
            if (!future.isSuccess()) {
                log.debug("Backend connect to {}:{} failed: {}", remoteHost, remotePort,
                        future.cause() == null ? "unknown" : future.cause().toString());
                ctx.close();
            }
        });
    }

    @Override
    public void channelRead(ChannelHandlerContext ctx, Object msg) {
        if (outboundActive) {
            outboundChannel.writeAndFlush(msg).addListener((ChannelFutureListener) f -> {
                if (!f.isSuccess()) {
                    closeOutbound();
                    ctx.close();
                }
            });
        } else {
            // Backend not connected yet: buffer the bytes.
            if (msg instanceof ByteBuf buf) {
                pending.add(buf.retainedDuplicate());
                buf.release();
            } else {
                pending.add(ctx.alloc().buffer().writeBytes((byte[]) msg));
            }
        }
    }

    @Override
    public void channelInactive(ChannelHandlerContext ctx) {
        closeOutbound();
        releasePending();
    }

    @Override
    public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) {
        // ECONNRESET / abrupt close: tear down both legs quietly.
        log.debug("Proxy exception on {}: {}", ctx.channel().remoteAddress(),
                cause.getClass().getSimpleName());
        closeOutbound();
        releasePending();
        ctx.close();
    }

    private void flushPending() {
        ByteBuf data;
        while ((data = pending.poll()) != null) {
            outboundChannel.writeAndFlush(data);
        }
    }

    private void closeOutbound() {
        if (closing) {
            return;
        }
        closing = true;
        if (outboundChannel != null && outboundChannel.isActive()) {
            outboundChannel.close();
        }
    }

    private void releasePending() {
        ByteBuf data;
        while ((data = pending.poll()) != null) {
            data.release();
        }
        if (initialData != null) {
            initialData.release();
            initialData = null;
        }
    }

    /**
     * Inbound handler for the outbound (backend) leg: flushes the initial bytes
     * once connected, then mirrors backend traffic back to the client channel.
     */
    private final class BackendHandler extends ChannelInboundHandlerAdapter {

        @Override
        public void channelActive(ChannelHandlerContext ctx) {
            outboundActive = true;
            if (initialData != null && initialData.isReadable()) {
                ctx.writeAndFlush(initialData.retainedDuplicate());
                initialData.release();
                initialData = null; // ownership handed to the write; never release again
            }
            flushPending();
            ctx.read();
        }

        @Override
        public void channelRead(ChannelHandlerContext ctx, Object msg) {
            if (inboundChannel != null && inboundChannel.isActive()) {
                // AUTO_READ is off on the outbound leg: re-arm a read after each
                // successful write, otherwise keep-alive connections hang after
                // the first response.
                inboundChannel.writeAndFlush(msg).addListener((ChannelFutureListener) f -> {
                    if (f.isSuccess()) {
                        ctx.read();
                    } else {
                        f.channel().close();
                    }
                });
            } else {
                ((ByteBuf) msg).release();
            }
        }

        @Override
        public void channelInactive(ChannelHandlerContext ctx) {
            outboundActive = false;
            if (inboundChannel != null && inboundChannel.isActive()) {
                inboundChannel.close();
            }
        }

        @Override
        public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) {
            log.debug("Backend exception: {}", cause.getClass().getSimpleName());
            ctx.close();
        }
    }
}
