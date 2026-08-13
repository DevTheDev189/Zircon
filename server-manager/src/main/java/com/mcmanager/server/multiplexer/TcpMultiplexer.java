package com.mcmanager.server.multiplexer;

import com.mcmanager.core.model.InstanceConfig;
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

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Binds the public Minecraft port (25565) and runs {@link ProtocolDetector} on
 * every accepted connection, proxying HTTP to the Javalin admin server and
 * Minecraft traffic to the internal port of the instance whose name/id matches
 * the handshake hostname (or the legacy single-server MC port when no instance
 * manager is wired).
 *
 * <p>Additionally binds one dedicated player-facing port per instance (see
 * {@link ServerInstanceManager#EXTERNAL_PORT_BASE}) so every server has a fixed,
 * memorable address; those listeners route straight to the instance's internal
 * port and are bound/unbound as instances are created/deleted via
 * {@link ServerInstanceManager.PortBindingListener}.
 */
public class TcpMultiplexer implements ServerInstanceManager.PortBindingListener {

    private static final Logger log = LoggerFactory.getLogger(TcpMultiplexer.class);

    private final ConfigService configService;
    private final ServerInstanceManager instanceManager; // nullable → legacy single-server
    private final String webHost;
    private final int webPort;
    private final String mcHost;
    private final int mcPort;
    /** Per-instance external-port listeners, keyed by instance id. */
    private final Map<String, Channel> instanceChannels = new ConcurrentHashMap<>();

    private EventLoopGroup bossGroup;
    private EventLoopGroup workerGroup;
    private Channel serverChannel;

    public TcpMultiplexer(ConfigService configService) {
        this(configService, null);
    }

    public TcpMultiplexer(ConfigService configService, ServerInstanceManager instanceManager) {
        this.configService = configService;
        this.instanceManager = instanceManager;
        ConfigService.ServerConfig cfg = configService.getConfig();
        this.webHost = "127.0.0.1";
        this.webPort = cfg.webPort;
        this.mcHost = "127.0.0.1";
        this.mcPort = cfg.mcPort;
    }

    @Override
    public void onInstanceAdded(InstanceConfig config) {
        bindInstance(config);
    }

    @Override
    public void onInstanceUpdated(InstanceConfig config) {
        // Manual port changes: drop the old listener, bind the new one.
        unbindInstance(config.getId());
        bindInstance(config);
    }

    @Override
    public void onInstanceRemoved(String instanceId) {
        unbindInstance(instanceId);
    }

    public void start() throws InterruptedException {
        ConfigService.ServerConfig cfg = configService.getConfig();

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
                                instanceManager, cfg.publicPort));
                    }
                });

        serverChannel = bootstrap.bind(cfg.publicPort).sync().channel();
        String mcTarget = instanceManager != null
                ? "MC -> instance-by-hostname (default " + mcHost + ":" + mcPort + ")"
                : "MC -> " + mcHost + ":" + mcPort;
        log.info("TCP multiplexer listening on 0.0.0.0:{} (HTTP -> {}:{}, {})",
                cfg.publicPort, webHost, webPort, mcTarget);

        // Bind one dedicated player-facing port per existing instance.
        if (instanceManager != null) {
            for (InstanceConfig inst : instanceManager.listInstances()) {
                bindInstance(inst);
            }
        }
    }

    /** Binds a dedicated player-facing port proxying to the instance's internal MC port. */
    public void bindInstance(InstanceConfig config) {
        if (config.getExternalMcPort() <= 0) {
            log.warn("Instance '{}' has no external port assigned; skipping port binding", config.getName());
            return;
        }
        if (config.getExternalMcPort() == configService.getConfig().publicPort) {
            // The main multiplexer listener already owns this port and routes to
            // the active instance (hostname match or fallback) — no extra binding.
            log.info("Instance '{}' uses the main multiplexer port {} (served by the public listener)",
                    config.getName(), config.getExternalMcPort());
            return;
        }
        if (instanceChannels.containsKey(config.getId())) {
            return; // already bound
        }
        ServerBootstrap bootstrap = new ServerBootstrap()
                .group(bossGroup, workerGroup)
                .channel(NioServerSocketChannel.class)
                .option(ChannelOption.SO_BACKLOG, 128)
                .childOption(ChannelOption.TCP_NODELAY, true)
                .childHandler(new ChannelInitializer<SocketChannel>() {
                    @Override
                    protected void initChannel(SocketChannel ch) {
                        ch.pipeline().addLast(new ProtocolDetector(webHost, webPort, mcHost, mcPort,
                                instanceManager, config));
                    }
                });
        try {
            Channel channel = bootstrap.bind(config.getExternalMcPort()).sync().channel();
            instanceChannels.put(config.getId(), channel);
            log.info("Bound external port {} -> instance '{}' (internal {})",
                    config.getExternalMcPort(), config.getName(), config.getInternalMcPort());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            log.warn("Interrupted while binding external port {} for instance '{}'",
                    config.getExternalMcPort(), config.getName());
        } catch (Exception e) {
            log.warn("Failed to bind external port {} for instance '{}': {}",
                    config.getExternalMcPort(), config.getName(), e.getMessage());
        }
    }

    /** Unbinds the dedicated player-facing port of an instance, if bound. */
    public void unbindInstance(String instanceId) {
        Channel channel = instanceChannels.remove(instanceId);
        if (channel != null) {
            channel.close().awaitUninterruptibly();
            log.info("Unbound external port for instance {}", instanceId);
        }
    }

    public void stop() {
        for (Channel channel : instanceChannels.values()) {
            channel.close().awaitUninterruptibly();
        }
        instanceChannels.clear();
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
