//! Glue that ties the DNS server, container watcher, and reverse proxy to
//! the app's settings + lifecycle.
//!
//! Owns the live `Arc<RwLock<RouteMap>>` plus handles for the running
//! resolver / watcher tasks. The reverse proxy will hang off the same
//! struct in a follow-up commit.

use std::sync::Arc;

use anyhow::Result;
use gpui::{App, Global};
use parking_lot::Mutex;

use crate::services::Tokio;

use super::route_map::{SharedRouteMap, new_shared};
use super::server::{DnsServerHandle, DocksideDnsHandler, spawn as spawn_server};
use super::watcher::{SharedDockerClient, WatcherHandle, spawn as spawn_watcher};

/// Live DNS+watcher state. Drop to stop everything.
pub struct DnsRuntime {
  pub routes: SharedRouteMap,
  server: DnsServerHandle,
  _watcher: WatcherHandle,
}

#[derive(Default)]
pub struct DnsManager {
  current: Mutex<Option<DnsRuntime>>,
}

impl DnsManager {
  pub fn new() -> Self {
    Self::default()
  }

  /// Returns the currently active route map, if the runtime is up. Used by
  /// other modules (Settings UI, Dashboard route tile) to read live state.
  pub fn route_map(&self) -> Option<SharedRouteMap> {
    self.current.lock().as_ref().map(|r| r.routes.clone())
  }

  pub fn is_running(&self) -> bool {
    self.current.lock().is_some()
  }

  /// Bound DNS port (matches `AppSettings::dns_port` unless the OS picked a
  /// different one because of a conflict). `None` when not running.
  pub fn bound_port(&self) -> Option<u16> {
    self.current.lock().as_ref().map(|r| r.server.bound_port)
  }

  /// Start (or restart) the DNS server + container watcher. `suffix` and
  /// `port` come from `AppSettings`. Safe to call repeatedly; the previous
  /// runtime is dropped before spawning the new one.
  pub fn start(&self, docker: SharedDockerClient, suffix: &str, port: u16) -> Result<()> {
    let routes = new_shared();
    let handle = Tokio::runtime_handle();
    let server_handle = handle.block_on(async {
      let handler = DocksideDnsHandler::new(suffix, routes.clone());
      spawn_server(handler, port).await
    })?;
    let watcher_handle = {
      let _enter = handle.enter();
      spawn_watcher(docker, routes.clone())
    };

    let prev = self.current.lock().replace(DnsRuntime {
      routes,
      server: server_handle,
      _watcher: watcher_handle,
    });
    drop(prev);
    Ok(())
  }

  /// Stop the runtime if running.
  pub fn stop(&self) {
    let _ = self.current.lock().take();
  }
}

/// Global wrapper so the rest of the app can find the manager.
struct GlobalDnsManager(Arc<DnsManager>);

impl Global for GlobalDnsManager {}

pub fn init(cx: &mut App) {
  cx.set_global(GlobalDnsManager(Arc::new(DnsManager::new())));
}

pub fn manager(cx: &App) -> Arc<DnsManager> {
  cx.global::<GlobalDnsManager>().0.clone()
}
