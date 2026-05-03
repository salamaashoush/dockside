//! Lifecycle wrapper around the reverse-proxy listeners (HTTP + HTTPS).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use gpui::{App, Global};
use parking_lot::Mutex;
use rustls::ServerConfig;

use crate::services::Tokio;
use crate::services::dns::SharedRouteMap;
use crate::services::tls::{DocksideCertResolver, LocalCa};

use super::server::{BindError, ProxyConfig, ProxyHandle, ProxyMode, spawn as spawn_proxy, spawn_https};

/// Unprivileged ports we degrade to when the running process can't bind
/// the user-configured low port. Mirrors the application defaults.
const FALLBACK_HTTP_PORT: u16 = 47080;
const FALLBACK_HTTPS_PORT: u16 = 47443;

pub struct ProxyRuntime {
  http: ProxyHandle,
  https: Option<ProxyHandle>,
}

#[derive(Default)]
pub struct ProxyManager {
  current: Mutex<Option<ProxyRuntime>>,
  /// True when at least one listener fell back to an unprivileged port
  /// after `EACCES` on a privileged port. The UI surfaces this so the
  /// user knows to relaunch — `setcap` only affects processes started
  /// after the grant.
  restart_required: AtomicBool,
}

impl ProxyManager {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn is_running(&self) -> bool {
    self.current.lock().is_some()
  }

  /// Bound (HTTP, HTTPS) ports if running. Either may equal `None` if the
  /// listener could not start (e.g. port already in use).
  pub fn bound_ports(&self) -> Option<(u16, Option<u16>)> {
    let guard = self.current.lock();
    guard
      .as_ref()
      .map(|r| (r.http.bound_port, r.https.as_ref().map(|h| h.bound_port)))
  }

  /// True when at least one listener degraded to an unprivileged
  /// fallback port after `EACCES`. The Settings panel renders a banner
  /// telling the user to relaunch.
  pub fn restart_required(&self) -> bool {
    self.restart_required.load(Ordering::SeqCst)
  }

  pub fn start(&self, suffix: &str, plain_port: u16, secure_port: u16, routes: SharedRouteMap) -> Result<()> {
    let handle = Tokio::runtime_handle();
    let suffix_arc: Arc<str> = {
      let mut s = suffix.trim().trim_matches('.').to_ascii_lowercase();
      s.push('.');
      Arc::from(s.as_str())
    };

    // Drop the previous runtime first so its ports are released before
    // we rebind. Without this, a fallback to FALLBACK_HTTP_PORT would
    // collide with the still-live old listener that's already bound to
    // that port.
    drop(self.current.lock().take());

    let mut restart_required = false;

    // HTTPS first — its bound port is the redirect target for plain
    // HTTP, so we have to know it before starting the plain listener.
    let (secure_handle, effective_https_port): (Option<ProxyHandle>, u16) = match build_tls_config(suffix_arc.clone()) {
      Ok(server_cfg) => {
        let cfg = ProxyConfig {
          suffix: suffix_arc.clone(),
          routes: routes.clone(),
          https_port: secure_port,
          mode: ProxyMode::Https,
        };
        let first = handle
          .block_on(async { spawn_https(([127, 0, 0, 1], secure_port).into(), cfg.clone(), server_cfg.clone()).await });
        match first {
          Ok(h) => {
            let p = h.bound_port;
            (Some(h), p)
          }
          Err(BindError::PermissionDenied(_)) if secure_port < 1024 => {
            tracing::warn!(
              "proxy https: bind on :{secure_port} denied — falling back to :{FALLBACK_HTTPS_PORT}. Restart Dockside to use the privileged port."
            );
            match handle
              .block_on(async { spawn_https(([127, 0, 0, 1], FALLBACK_HTTPS_PORT).into(), cfg, server_cfg).await })
            {
              Ok(h) => {
                restart_required = true;
                let p = h.bound_port;
                (Some(h), p)
              }
              Err(e) => {
                tracing::warn!("proxy https: fallback bind failed ({e}); HTTP-only mode");
                (None, secure_port)
              }
            }
          }
          Err(e) => {
            tracing::warn!("proxy https: failed to start ({e}); HTTP-only mode");
            (None, secure_port)
          }
        }
      }
      Err(e) => {
        tracing::warn!("proxy https: TLS init failed ({e}); HTTP-only mode");
        (None, secure_port)
      }
    };

    let plain_cfg = ProxyConfig {
      suffix: suffix_arc,
      routes,
      https_port: effective_https_port,
      mode: ProxyMode::Http,
    };
    let plain_handle = match handle
      .block_on(async { spawn_proxy(([127, 0, 0, 1], plain_port).into(), plain_cfg.clone()).await })
    {
      Ok(h) => h,
      Err(BindError::PermissionDenied(_)) if plain_port < 1024 => {
        tracing::warn!(
          "proxy http: bind on :{plain_port} denied — falling back to :{FALLBACK_HTTP_PORT}. Restart Dockside to use the privileged port."
        );
        let h = handle
          .block_on(async { spawn_proxy(([127, 0, 0, 1], FALLBACK_HTTP_PORT).into(), plain_cfg).await })
          .map_err(|e| match e {
            BindError::PermissionDenied(a) => anyhow::anyhow!("proxy http: permission denied binding {a}"),
            BindError::Other(err) => err,
          })?;
        restart_required = true;
        h
      }
      Err(BindError::PermissionDenied(a)) => {
        return Err(anyhow::anyhow!("proxy http: permission denied binding {a}"));
      }
      Err(BindError::Other(e)) => return Err(e),
    };

    self.current.lock().replace(ProxyRuntime {
      http: plain_handle,
      https: secure_handle,
    });
    self.restart_required.store(restart_required, Ordering::SeqCst);
    Ok(())
  }

  pub fn stop(&self) {
    let _ = self.current.lock().take();
    self.restart_required.store(false, Ordering::SeqCst);
  }
}

fn build_tls_config(suffix: Arc<str>) -> Result<Arc<ServerConfig>> {
  let ca = Arc::new(LocalCa::load_or_create().context("load/create local CA")?);
  let resolver = Arc::new(DocksideCertResolver::new(ca, suffix));

  // Make sure ring is the active provider.
  rustls::crypto::ring::default_provider().install_default().ok();

  let config = ServerConfig::builder()
    .with_no_client_auth()
    .with_cert_resolver(resolver);
  Ok(Arc::new(config))
}

struct GlobalProxyManager(Arc<ProxyManager>);

impl Global for GlobalProxyManager {}

pub fn init(cx: &mut App) {
  cx.set_global(GlobalProxyManager(Arc::new(ProxyManager::new())));
}

pub fn manager(cx: &App) -> Arc<ProxyManager> {
  cx.global::<GlobalProxyManager>().0.clone()
}
