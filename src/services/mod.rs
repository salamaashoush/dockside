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
/// PEM is read from `<config>/dockside/ca/root.crt`. Generated on first
/// call so the user can install before ever enabling DNS.
pub fn install_local_ca() -> anyhow::Result<()> {
  // Make sure the CA exists on disk first; load_or_create() generates it on
  // first call and is a no-op afterwards.
  let _ca = tls::LocalCa::load_or_create()?;
  let path = local_ca_pem_path()?;
  let path_str = path
    .to_str()
    .ok_or_else(|| anyhow::anyhow!("CA PEM path is not valid UTF-8"))?;
  helper::run_privileged(&["install-ca", path_str]).map(|_| ())
}

pub fn uninstall_local_ca() -> anyhow::Result<()> {
  helper::run_privileged(&["uninstall-ca"]).map(|_| ())
}

/// One-time setup: copy helper to `/usr/local/libexec/`, write polkit rule
/// (so future operations don't need an auth prompt each time), install the
/// resolver, and trust the local CA — all in a single pkexec invocation.
pub fn bootstrap(suffix: &str, dns_port: u16) -> anyhow::Result<()> {
  // Make sure the CA file exists so the helper can copy it into the trust
  // store as part of bootstrap. No-op if already generated.
  let _ca = tls::LocalCa::load_or_create()?;
  let pem = local_ca_pem_path()?;
  let pem_str = pem
    .to_str()
    .ok_or_else(|| anyhow::anyhow!("CA PEM path is not valid UTF-8"))?;
  let port_str = dns_port.to_string();
  helper::run_privileged(&["bootstrap", suffix, &port_str, pem_str]).map(|_| ())
}

pub fn uninstall_bootstrap() -> anyhow::Result<()> {
  helper::run_privileged(&["uninstall-bootstrap"]).map(|_| ())
}

/// Did the user already run the bootstrap step? Probes the polkit policy
/// file presence — cheap and matches what `bootstrap_linux` writes.
pub fn is_bootstrapped() -> bool {
  #[cfg(target_os = "linux")]
  {
    std::path::Path::new("/usr/share/polkit-1/actions/dev.dockside.helper.policy").is_file()
      && std::path::Path::new("/usr/local/libexec/dockside-helper").is_file()
  }
  #[cfg(not(target_os = "linux"))]
  {
    // macOS doesn't need a separate bootstrap step — the resolver install
    // already provisions everything. Treat as "ready" if the resolver file
    // exists (cheap heuristic; we don't currently track it explicitly).
    true
  }
}

/// Check whether the Dockside root CA is currently installed in the OS
/// trust store. Pure file-existence probe — no privileged call needed.
pub fn is_local_ca_installed() -> bool {
  #[cfg(target_os = "linux")]
  {
    std::path::Path::new("/etc/ca-certificates/trust-source/anchors/dockside.crt").is_file()
      || std::path::Path::new("/usr/local/share/ca-certificates/dockside.crt").is_file()
  }
  #[cfg(target_os = "macos")]
  {
    // `security find-certificate` exits 0 when the cert is in the keychain.
    std::process::Command::new("/usr/bin/security")
      .args([
        "find-certificate",
        "-c",
        "Dockside Local Root CA",
        "/Library/Keychains/System.keychain",
      ])
      .output()
      .map(|o| o.status.success())
      .unwrap_or(false)
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos")))]
  {
    false
  }
}

/// Build a `<name>.<suffix>:<http_port>` URL for the given container id, or
/// `None` if DNS is disabled / the container is not yet routed. Returns the
/// full `http://...` URL (HTTPS would need the user to install the root CA;
/// we surface plain HTTP for now since it works without trust-store setup).
pub fn container_url(cx: &App, container_id: &str) -> Option<String> {
  use crate::state::settings_state;
  let settings = settings_state(cx).read(cx).settings.clone();
  if !settings.dns_enabled {
    return None;
  }
  let map = dns::manager(cx).route_map()?;
  let host = map.read().primary_for_container(container_id)?;
  let suffix = settings.dns_suffix.trim_matches('.');
  let port = settings.proxy_http_port;
  let url = if port == 80 {
    format!("http://{host}.{suffix}/")
  } else {
    format!("http://{host}.{suffix}:{port}/")
  };
  Some(url)
}

fn local_ca_pem_path() -> anyhow::Result<std::path::PathBuf> {
  let base = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
  Ok(base.join("dockside").join("ca").join("root.crt"))
}
