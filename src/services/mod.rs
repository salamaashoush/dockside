//! Application services layer
//!
//! This module contains all the async operations and dispatchers for the application.
//! It is organized into submodules by resource type:
//!
//! - `core` - Dispatcher types and Docker client management
//! - `docker` - Docker resource operations (containers, images, volumes, networks, compose)
//! - `colima` - Colima machine and Kubernetes control operations
//! - `kubernetes` - Kubernetes resource operations (pods, services, deployments)
//! - `navigation` - View and tab navigation functions
//! - `prune` - Docker prune operations
//! - `init` - Initial data loading
//! - `watchers` - Real-time resource watchers for Docker and Kubernetes

mod colima;
mod core;
pub mod dns;
mod docker;
mod favorites;
mod gpui_tokio;
pub mod helper;
mod host;
mod init;
mod kubernetes;
mod navigation;
pub mod proxy;
mod prune;
mod task_manager;
pub mod tls;
mod watchers;

// Re-export everything for backward compatibility
pub use colima::*;
pub use core::*;
pub use docker::*;
pub use favorites::*;
pub use gpui_tokio::Tokio;
pub use host::*;
pub use init::*;
pub use kubernetes::*;
pub use navigation::*;
pub use prune::*;
pub use task_manager::*;
pub use watchers::stop_watchers;

use gpui::App;

use crate::state::{init_docker_state, init_settings};

/// Initialize all global services
pub fn init_services(cx: &mut App) {
  // Initialize tokio runtime first (required for Docker client)
  gpui_tokio::init(cx);

  // Initialize state
  init_docker_state(cx);
  init_settings(cx);

  // Initialize services
  init_task_manager(cx);
  init_dispatcher(cx);
  dns::init(cx);
  proxy::init(cx);
}

/// Start the real-time resource watchers
///
/// Call this after initial data loading to enable real-time updates.
/// The watchers monitor Docker events and Kubernetes resource changes,
/// automatically refreshing the UI when resources are added, modified, or deleted.
pub fn start_watchers(cx: &mut App) {
  let docker_client = docker_client();
  watchers::start_watchers(docker_client, cx);
  apply_dns_settings(cx);
}

/// Apply the current `AppSettings` DNS toggles. Starts or stops the local
/// resolver + container watcher + reverse proxy accordingly. Safe to call
/// any time the user changes the DNS settings.
pub fn apply_dns_settings(cx: &mut App) {
  use crate::state::settings_state;
  let settings = settings_state(cx).read(cx).settings.clone();
  let dns_mgr = dns::manager(cx);
  let proxy_mgr = proxy::manager(cx);
  if settings.dns_enabled {
    let client = docker_client();
    if let Err(e) = dns_mgr.start(client, &settings.dns_suffix, settings.dns_port) {
      tracing::warn!("dns: failed to start: {e}");
      return;
    }
    let Some(routes) = dns_mgr.route_map() else {
      tracing::warn!("dns: no route map after start");
      return;
    };
    if let Err(e) = proxy_mgr.start(
      &settings.dns_suffix,
      settings.proxy_http_port,
      settings.proxy_https_port,
      routes,
    ) {
      tracing::warn!("proxy: failed to start: {e}");
    }
  } else {
    proxy_mgr.stop();
    dns_mgr.stop();
  }
}

/// Install the per-domain resolver entry into the OS via the privileged
/// helper. Triggered from the Settings UI's "Enable" toggle.
pub fn install_dns_resolver(suffix: &str, port: u16) -> anyhow::Result<()> {
  let port_str = port.to_string();
  helper::run_privileged(&["install-resolver", suffix, &port_str]).map(|_| ())
}

pub fn uninstall_dns_resolver(suffix: &str) -> anyhow::Result<()> {
  helper::run_privileged(&["uninstall-resolver", suffix]).map(|_| ())
}

/// Install the local CA certificate into the OS trust store. The CA cert
/// PEM is read from `<config>/dockside/ca/root.crt` which the proxy
/// runtime ensures exists (created on first HTTPS spin-up).
pub fn install_local_ca() -> anyhow::Result<()> {
  let path = local_ca_pem_path()?;
  let path_str = path
    .to_str()
    .ok_or_else(|| anyhow::anyhow!("CA PEM path is not valid UTF-8"))?;
  helper::run_privileged(&["install-ca", path_str]).map(|_| ())
}

pub fn uninstall_local_ca() -> anyhow::Result<()> {
  helper::run_privileged(&["uninstall-ca"]).map(|_| ())
}

fn local_ca_pem_path() -> anyhow::Result<std::path::PathBuf> {
  let base = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
  Ok(base.join("dockside").join("ca").join("root.crt"))
}
