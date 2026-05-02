//! Lifecycle wrapper around the reverse-proxy listeners (HTTP + HTTPS).

use std::sync::Arc;

use anyhow::{Context, Result};
use gpui::{App, Global};
use parking_lot::Mutex;
use rustls::ServerConfig;

use crate::services::Tokio;
use crate::services::dns::SharedRouteMap;
use crate::services::tls::{DocksideCertResolver, LocalCa};

use super::server::{ProxyConfig, ProxyHandle, ProxyMode, spawn as spawn_proxy, spawn_https};

pub struct ProxyRuntime {
  http: ProxyHandle,
  https: Option<ProxyHandle>,
}

#[derive(Default)]
pub struct ProxyManager {
  current: Mutex<Option<ProxyRuntime>>,
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

  pub fn start(&self, suffix: &str, plain_port: u16, secure_port: u16, routes: SharedRouteMap) -> Result<()> {
    let handle = Tokio::runtime_handle();
    let suffix_arc: Arc<str> = {
      let mut s = suffix.trim().trim_matches('.').to_ascii_lowercase();
      s.push('.');
      Arc::from(s.as_str())
    };

    let plain_handle = handle.block_on(async {
      let cfg = ProxyConfig {
        suffix: suffix_arc.clone(),
        routes: routes.clone(),
        https_port: secure_port,
        mode: ProxyMode::Http,
      };
      spawn_proxy(([127, 0, 0, 1], plain_port).into(), cfg).await
    })?;

    // HTTPS listener — only spin up if the local CA is available.
    let secure_handle = match build_tls_config(suffix_arc.clone()) {
      Ok(server_cfg) => {
        let cfg = ProxyConfig {
          suffix: suffix_arc,
          routes,
          https_port: secure_port,
          mode: ProxyMode::Https,
        };
        match handle.block_on(async { spawn_https(([127, 0, 0, 1], secure_port).into(), cfg, server_cfg).await }) {
          Ok(h) => Some(h),
          Err(e) => {
            tracing::warn!("proxy https: failed to start ({e}); HTTP-only mode");
            None
          }
        }
      }
      Err(e) => {
        tracing::warn!("proxy https: TLS init failed ({e}); HTTP-only mode");
        None
      }
    };

    let prev = self.current.lock().replace(ProxyRuntime {
      http: plain_handle,
      https: secure_handle,
    });
    drop(prev);
    Ok(())
  }

  pub fn stop(&self) {
    let _ = self.current.lock().take();
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
