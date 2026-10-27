//! Binds the public Minecraft port (25565) and runs protocol detection on every
//! accepted connection, proxying HTTP to the admin web server and Minecraft
//! traffic to the internal port of the instance whose name/id matches the
//! handshake hostname (or the legacy single-server MC port when no instance
//! manager is wired).
//!
//! Additionally binds one dedicated player-facing port per instance so every
//! server has a fixed, memorable address; those listeners route straight to the
//! instance's internal port and are bound/unbound as instances are
//! created/deleted via `PortBindingListener`.
//!
//! Port of `com.mcmanager.server.multiplexer.TcpMultiplexer` + `ProxyHandler`.

pub mod handshake;
pub mod proxy;
#[cfg(test)]
mod tests;

pub(crate) use proxy::*;

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;
use zircon_core::model::InstanceConfig;

use crate::config::ConfigService;
use crate::instance::{PortBindingListener, ServerInstanceManager};
use crate::tickets::JoinTicketManager;

const PUBLIC_LISTENER_KEY: &str = "__public_listener__";

/// The Tokio TCP multiplexer.
#[derive(Clone)]
pub struct TcpMultiplexer {
    pub(crate) config: Arc<ConfigService>,
    pub(crate) instances: Option<Arc<ServerInstanceManager>>,
    pub(crate) tickets: Arc<JoinTicketManager>,
    pub(crate) web_port: u16,
    pub(crate) mc_port: u16,
    pub(crate) bindings: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    /// Global connection budget: an owned permit is held for every accepted
    /// connection and released when its task ends.
    pub(crate) global_limiter: Arc<tokio::sync::Semaphore>,
    /// Currently accepted connections per source IP, for the per-IP quota.
    pub(crate) ip_conns: Arc<dashmap::DashMap<std::net::IpAddr, usize>>,
}

impl TcpMultiplexer {
    pub fn new(
        config: Arc<ConfigService>,
        instances: Option<Arc<ServerInstanceManager>>,
        tickets: Arc<JoinTicketManager>,
    ) -> Self {
        let cfg = config.get_config();
        Self {
            config,
            instances,
            tickets,
            web_port: cfg.web_port as u16,
            mc_port: cfg.mc_port as u16,
            bindings: Arc::new(Mutex::new(HashMap::new())),
            global_limiter: Arc::new(tokio::sync::Semaphore::new(MAX_GLOBAL_TCP_CONNS)),
            ip_conns: Arc::new(dashmap::DashMap::new()),
        }
    }

    /// Binds the public port and one dedicated player-facing port per existing
    /// instance.
    pub fn start(&self) -> io::Result<()> {
        let public_port = self.config.get_config().public_port as u16;
        let handle = self.spawn_listener(public_port, None);
        if let Some(old) = self
            .bindings
            .lock()
            .unwrap()
            .insert(PUBLIC_LISTENER_KEY.to_string(), handle)
        {
            old.abort();
        }
        if let Some(instances) = &self.instances {
            for instance in instances.list_instances() {
                self.bind_instance(&instance);
            }
        }
        Ok(())
    }

    /// Binds a dedicated player-facing port proxying to the instance's internal
    /// MC port.
    pub fn bind_instance(&self, config: &InstanceConfig) {
        if config.external_mc_port <= 0 {
            tracing::warn!(
                "Instance '{}' has no external port assigned; skipping port binding",
                config.name
            );
            return;
        }
        if config.external_mc_port == self.config.get_config().public_port {
            // The main multiplexer listener already owns this port and routes to
            // the active instance (hostname match or fallback).
            tracing::info!(
                "Instance '{}' uses the main multiplexer port {} (served by the public listener)",
                config.name,
                config.external_mc_port
            );
            return;
        }
        if self.bindings.lock().unwrap().contains_key(&config.id) {
            return; // already bound
        }
        tracing::info!(
            "Bound external port {} -> instance '{}' (internal {})",
            config.external_mc_port,
            config.name,
            config.internal_mc_port
        );
        let handle = self.spawn_listener(config.external_mc_port as u16, Some(config.clone()));
        self.bindings
            .lock()
            .unwrap()
            .insert(config.id.clone(), handle);
    }

    /// Unbinds the dedicated player-facing port of an instance, if bound.
    pub fn unbind_instance(&self, instance_id: &str) {
        if let Some(handle) = self.bindings.lock().unwrap().remove(instance_id) {
            handle.abort();
            tracing::info!("Unbound external port for instance {instance_id}");
        }
    }

    /// Stops all listeners.
    pub fn stop(&self) {
        let handles: Vec<JoinHandle<()>> = self
            .bindings
            .lock()
            .unwrap()
            .drain()
            .map(|(_, h)| h)
            .collect();
        for handle in handles {
            handle.abort();
        }
    }


}

impl PortBindingListener for TcpMultiplexer {
    fn on_instance_added(&self, config: &InstanceConfig) {
        self.bind_instance(config);
    }

    fn on_instance_updated(&self, config: &InstanceConfig) {
        // Manual port changes: drop the old listener, bind the new one.
        self.unbind_instance(&config.id);
        self.bind_instance(config);
    }

    fn on_instance_removed(&self, instance_id: &str) {
        self.unbind_instance(instance_id);
    }
}


