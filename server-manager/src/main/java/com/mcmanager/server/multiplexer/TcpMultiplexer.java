package com.mcmanager.server.multiplexer;

import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.service.ConfigService;
import io.netty.bootstrap.ServerBootstrap;
import io.netty.channel.Channel;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.ChannelOption;
import io.netty.channel.EventLoopGroup;
import io.netty.channel.nio.NioEventLoopGroup;
import io.netty.channel.socket.SocketChannel;
import io.netty.channel.socket.nio.NioServerSocketChannel;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Binds the public Minecraft port (25565) and runs {@link ProtocolDetector} on
 * every accepted connection, proxying HTTP to the Javalin admin server and
 * Minecraft traffic to the internal port of the instance whose name/id matches
 * the handshake hostname (or the legacy single-server MC port when no instance
 * manager is wired).
 */
public class TcpMultiplexer {

    private static final Logger log = LoggerFactory.getLogger(TcpMultiplexer.class);

    private final ConfigService configService;
    private final ServerInstanceManager instanceManager; // nullable → legacy single-server
    private EventLoopGroup bossGroup;
    private EventLoopGroup workerGroup;
    private Channel serverChannel;

    public TcpMultiplexer(ConfigService configService) {
        this(configService, null);
    }

    public TcpMultiplexer(ConfigService configService, ServerInstanceManager instanceManager) {
        this.configService = configService;
        this.instanceManager = instanceManager;
    }

    public void start() throws InterruptedException {
        ConfigService.ServerConfig cfg = configService.getConfig();
        String webHost = "127.0.0.1";
        int webPort = cfg.webPort;
        String mcHost = "127.0.0.1";
        int mcPort = cfg.mcPort;

        bossGroup = new NioEventLoopGroup(1);
        workerGroup = new NioEventLoopGroup();

        ServerBootstrap bootstrap = new ServerBootstrap()
                .group(bossGroup, workerGroup)
                .channel(NioServerSocketChannel.class)
                .option(ChannelOption.SO_BACKLOG, 128)
                .childOption(ChannelOption.TCP_NODELAY, true)
                .childHandler(new ChannelInitializer<SocketChannel>() {
                    @Override
                    protected void initChannel(SocketChannel ch) {
                        ch.pipeline().addLast(new ProtocolDetector(webHost, webPort, mcHost, mcPort,
                                instanceManager));
                    }
                });

        serverChannel = bootstrap.bind(cfg.publicPort).sync().channel();
        String mcTarget = instanceManager != null
                ? "MC -> instance-by-hostname (default " + mcHost + ":" + mcPort + ")"
                : "MC -> " + mcHost + ":" + mcPort;
        log.info("TCP multiplexer listening on 0.0.0.0:{} (HTTP -> {}:{}, {})",
                cfg.publicPort, webHost, webPort, mcTarget);
    }

    public void stop() {
        if (serverChannel != null) {
            serverChannel.close().awaitUninterruptibly();
        }
        if (bossGroup != null) {
            bossGroup.shutdownGracefully();
        }
        if (workerGroup != null) {
            workerGroup.shutdownGracefully();
        }
        log.info("TCP multiplexer stopped");
    }
}
