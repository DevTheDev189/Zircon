package com.mcmanager.server.multiplexer;

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
 * everything else to the internal Minecraft server port.
 */
public class TcpMultiplexer {

    private static final Logger log = LoggerFactory.getLogger(TcpMultiplexer.class);

    private final ConfigService configService;
    private EventLoopGroup bossGroup;
    private EventLoopGroup workerGroup;
    private Channel serverChannel;

    public TcpMultiplexer(ConfigService configService) {
        this.configService = configService;
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
                        ch.pipeline().addLast(new ProtocolDetector(webHost, webPort, mcHost, mcPort));
                    }
                });

        serverChannel = bootstrap.bind(cfg.publicPort).sync().channel();
        log.info("TCP multiplexer listening on 0.0.0.0:{} (HTTP -> {}:{}, MC -> {}:{})",
                cfg.publicPort, webHost, webPort, mcHost, mcPort);
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
