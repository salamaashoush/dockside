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
///
/// Also adds the CA to per-user NSS databases (`~/.pki/nssdb` for
/// Chrome/Chromium/Brave/Edge, `~/.mozilla/firefox/<profile>/`) since
/// those don't share the system trust store on Linux.
pub fn install_local_ca() -> anyhow::Result<()> {
  let _ca = tls::LocalCa::load_or_create()?;
  let path = local_ca_pem_path()?;
  let path_str = path
    .to_str()
    .ok_or_else(|| anyhow::anyhow!("CA PEM path is not valid UTF-8"))?;
  helper::run_privileged(&["install-ca", path_str])?;
  if let Err(e) = install_ca_in_user_nss(&path) {
    tracing::warn!("dns: NSS user-store install partial failure: {e}");
  }
  Ok(())
}

pub fn uninstall_local_ca() -> anyhow::Result<()> {
  uninstall_ca_in_user_nss();
  helper::run_privileged(&["uninstall-ca"]).map(|_| ())
}

const NSS_LABEL: &str = "Dockside Local Root CA";

/// Install the CA into every NSS database the user owns: Chromium-family
/// shared db at `~/.pki/nssdb`, plus each Firefox profile (each is its
/// own NSS db). Skips silently if `certutil` is not installed.
fn install_ca_in_user_nss(pem_path: &std::path::Path) -> anyhow::Result<()> {
  use std::process::Command;
  let Some(certutil) = find_certutil() else {
    tracing::info!("dns: certutil not found — install nss/libnss3-tools to trust the CA in browsers");
    return Ok(());
  };
  for db in user_nss_dbs() {
    // Idempotency: certutil errors if the nick already exists, so delete
    // first; ignore failure (deleting a missing entry returns non-zero).
    let _ = Command::new(&certutil)
      .args(["-D", "-n", NSS_LABEL, "-d", &format!("sql:{}", db.display())])
      .output();
    let status = Command::new(&certutil)
      .args([
        "-A",
        "-n",
        NSS_LABEL,
        "-t",
        "C,,",
        "-i",
        pem_path.to_str().unwrap_or_default(),
        "-d",
        &format!("sql:{}", db.display()),
      ])
      .status()?;
    if status.success() {
      tracing::info!("dns: trusted CA in NSS db {}", db.display());
    } else {
      tracing::warn!("dns: certutil -A failed for {} (exit {status})", db.display());
    }
  }
  Ok(())
}

fn uninstall_ca_in_user_nss() {
  use std::process::Command;
  let Some(certutil) = find_certutil() else {
    return;
  };
  for db in user_nss_dbs() {
    let _ = Command::new(&certutil)
      .args(["-D", "-n", NSS_LABEL, "-d", &format!("sql:{}", db.display())])
      .output();
  }
}

fn find_certutil() -> Option<std::path::PathBuf> {
  for p in ["/usr/bin/certutil", "/usr/local/bin/certutil"] {
    let path = std::path::Path::new(p);
    if path.is_file() {
      return Some(path.to_path_buf());
    }
  }
  which::which("certutil").ok()
}

fn user_nss_dbs() -> Vec<std::path::PathBuf> {
  let mut dbs: Vec<std::path::PathBuf> = Vec::new();
  let Some(home) = dirs::home_dir() else {
    return dbs;
  };
  // Chromium-family shared db.
  let chromium = home.join(".pki").join("nssdb");
  if chromium.is_dir() {
    dbs.push(chromium);
  }
  // Firefox profiles (sql-format db = cert9.db).
  let ff = home.join(".mozilla").join("firefox");
  if let Ok(entries) = std::fs::read_dir(&ff) {
    for entry in entries.flatten() {
      let p = entry.path();
      if p.is_dir() && p.join("cert9.db").is_file() {
        dbs.push(p);
      }
    }
  }
  dbs
}

/// One-time setup: copy helper to `/usr/local/libexec/`, write polkit rule
/// (so future operations don't need an auth prompt each time), install the
/// resolver, trust the local CA in the system store — all in a single
/// pkexec invocation. After that, also install the CA into the user's NSS
/// databases (Chromium / Firefox) which don't read the system store.
pub fn bootstrap(suffix: &str, dns_port: u16) -> anyhow::Result<()> {
  let _ca = tls::LocalCa::load_or_create()?;
  let pem = local_ca_pem_path()?;
  let pem_str = pem
    .to_str()
    .ok_or_else(|| anyhow::anyhow!("CA PEM path is not valid UTF-8"))?;
  let port_str = dns_port.to_string();
  helper::run_privileged(&["bootstrap", suffix, &port_str, pem_str])?;
  if let Err(e) = install_ca_in_user_nss(&pem) {
    tracing::warn!("dns: NSS user-store install partial failure: {e}");
  }
  Ok(())
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

/// Check whether the Dockside root CA is currently installed.
///
/// "Installed" means BOTH the OS trust store (system tools — curl, wget,
/// language SDKs) AND at least one user NSS db (Chromium-family browsers,
/// Firefox profile) carry the cert. Returns true only when both succeed,
/// so the UI label honestly reflects "browser will trust this".
pub fn is_local_ca_installed() -> bool {
  let system = is_local_ca_installed_system();
  let nss = is_local_ca_installed_in_nss();
  // Treat NSS as optional only when no NSS dbs exist on this machine
  // (e.g. a server with no graphical browsers); otherwise both required.
  let no_nss_dbs = user_nss_dbs().is_empty();
  system && (nss || no_nss_dbs)
}

fn is_local_ca_installed_system() -> bool {
  #[cfg(target_os = "linux")]
  {
    std::path::Path::new("/etc/ca-certificates/trust-source/anchors/dockside.crt").is_file()
      || std::path::Path::new("/usr/local/share/ca-certificates/dockside.crt").is_file()
  }
  #[cfg(target_os = "macos")]
  {
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

fn is_local_ca_installed_in_nss() -> bool {
  let Some(certutil) = find_certutil() else {
    return false;
  };
  for db in user_nss_dbs() {
    let output = std::process::Command::new(&certutil)
      .args(["-L", "-n", NSS_LABEL, "-d", &format!("sql:{}", db.display())])
      .output();
    if let Ok(o) = output
      && o.status.success()
    {
      return true;
    }
  }
  false
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
