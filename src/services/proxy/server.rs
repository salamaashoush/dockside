//! HTTP/HTTPS reverse proxy listening on `127.0.0.1:80` (and `:443` once
//! TLS lands). Reads `Host:` (or SNI) → looks up `RouteMap` → forwards to
//! the matching container's `Backend`. WebSocket upgrades pass through.

use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::OnUpgrade;
use hyper::{Request, Response, StatusCode, Uri, header};
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type ProxyBody = BoxBody<Bytes, BoxError>;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::services::dns::{Backend, SharedRouteMap};

pub struct ProxyHandle {
  shutdown: Option<oneshot::Sender<()>>,
  pub bound_port: u16,
}

impl Drop for ProxyHandle {
  fn drop(&mut self) {
    if let Some(tx) = self.shutdown.take() {
      let _ = tx.send(());
    }
  }
}

/// Configuration for a single listener.
#[derive(Clone)]
pub struct ProxyConfig {
  pub suffix: Arc<str>,
  pub routes: SharedRouteMap,
  /// HTTPS listen port. The HTTP listener uses this to build redirect URLs
  /// when the local CA is trusted; otherwise plain HTTP is served as-is.
  pub https_port: u16,
  pub mode: ProxyMode,
}

#[derive(Clone, Copy)]
pub enum ProxyMode {
  /// Plain HTTP listener.
  Http,
  /// TLS-terminating listener using the local CA's cert resolver.
  Https,
}

/// `bind()` outcome. Caller distinguishes `PermissionDenied` so it can
/// fall back to an unprivileged port and surface a "restart required"
/// hint — Linux file capabilities only take effect for processes started
/// after the grant, so the running app cannot bind 80/443 even after
/// `setcap` succeeds on the binary.
#[derive(Debug)]
pub enum BindError {
  PermissionDenied(SocketAddr),
  Other(anyhow::Error),
}

impl std::fmt::Display for BindError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::PermissionDenied(addr) => write!(f, "permission denied binding {addr}"),
      Self::Other(e) => write!(f, "{e}"),
    }
  }
}

impl std::error::Error for BindError {}

pub async fn spawn(addr: SocketAddr, config: ProxyConfig) -> Result<ProxyHandle, BindError> {
  spawn_inner(addr, config, None).await
}

pub async fn spawn_https(
  addr: SocketAddr,
  config: ProxyConfig,
  server_cfg: Arc<ServerConfig>,
) -> Result<ProxyHandle, BindError> {
  spawn_inner(addr, config, Some(server_cfg)).await
}

async fn spawn_inner(
  addr: SocketAddr,
  config: ProxyConfig,
  tls: Option<Arc<ServerConfig>>,
) -> Result<ProxyHandle, BindError> {
  let listener = match TcpListener::bind(addr).await {
    Ok(l) => l,
    Err(e) if e.kind() == io::ErrorKind::PermissionDenied => return Err(BindError::PermissionDenied(addr)),
    Err(e) => {
      return Err(BindError::Other(
        anyhow::Error::new(e).context(format!("bind reverse proxy {addr}")),
      ));
    }
  };
  let bound_port = listener
    .local_addr()
    .map_err(|e| BindError::Other(anyhow::Error::new(e).context("query local_addr")))?
    .port();

  let mut connector = HttpConnector::new();
  connector.set_nodelay(true);
  connector.enforce_http(false);
  let client: Client<HttpConnector, ProxyBody> = Client::builder(TokioExecutor::new()).build(connector);

  let (tx, mut rx) = oneshot::channel::<()>();
  let cfg = Arc::new(config);
  let acceptor: Option<TlsAcceptor> = tls.map(TlsAcceptor::from);

  tokio::spawn(async move {
    loop {
      tokio::select! {
        _ = &mut rx => break,
        accept = listener.accept() => {
          let (stream, _) = match accept {
            Ok(pair) => pair,
            Err(e) => {
              tracing::warn!("proxy: accept error: {e}");
              continue;
            }
          };
          let cfg = cfg.clone();
          let client = client.clone();
          let acceptor = acceptor.clone();
          tokio::spawn(async move {
            match acceptor {
              Some(acceptor) => match acceptor.accept(stream).await {
                Ok(tls_stream) => serve_connection(TokioIo::new(tls_stream), cfg, client).await,
                Err(e) => tracing::debug!("proxy tls handshake error: {e}"),
              },
              None => serve_connection(TokioIo::new(stream), cfg, client).await,
            }
          });
        }
      }
    }
  });

  Ok(ProxyHandle {
    shutdown: Some(tx),
    bound_port,
  })
}

async fn serve_connection<I>(io: I, cfg: Arc<ProxyConfig>, client: Client<HttpConnector, ProxyBody>)
where
  I: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
{
  let svc = service_fn(move |req: Request<Incoming>| {
    let cfg = cfg.clone();
    let client = client.clone();
    async move { Ok::<_, Infallible>(handle_request(req, cfg, client).await) }
  });
  if let Err(e) = http1::Builder::new()
    .preserve_header_case(true)
    .serve_connection(io, svc)
    .with_upgrades()
    .await
  {
    tracing::debug!("proxy: connection error: {e}");
  }
}

async fn handle_request(
  mut req: Request<Incoming>,
  cfg: Arc<ProxyConfig>,
  client: Client<HttpConnector, ProxyBody>,
) -> Response<ProxyBody> {
  let Some(host) = extract_host(&req) else {
    return error_page(StatusCode::BAD_REQUEST, "Missing Host header");
  };
  let Some(label) = strip_suffix(&host, &cfg.suffix) else {
    return error_page(StatusCode::NOT_FOUND, "Unknown host");
  };

  let route = {
    let routes = cfg.routes.read();
    routes.lookup(&label).cloned()
  };
  let Some(route) = route else {
    return error_page(StatusCode::NOT_FOUND, &format!("No container named '{label}'"));
  };

  // HTTPS-only routes refuse plain HTTP outright.
  if matches!(cfg.mode, ProxyMode::Http) && route.https_only {
    return error_page(
      StatusCode::UPGRADE_REQUIRED,
      "This service is HTTPS-only. Reload over https://.",
    );
  }

  // Auto-redirect HTTP → HTTPS only when the local CA is in the trust
  // store. Before that, the redirect lands on a cert error and breaks the
  // user. Per-container `dockside.http_only=true` opts out entirely.
  if matches!(cfg.mode, ProxyMode::Http) && !route.http_only && crate::services::is_local_ca_installed() {
    return redirect_to_https(&req, &host, cfg.https_port);
  }

  let upstream = match upstream_authority(&route.backend) {
    Ok(s) => s,
    Err(reason) => {
      return error_page(StatusCode::BAD_GATEWAY, reason);
    }
  };

  // WebSocket upgrade: rebuild request, forward to backend, splice both
  // ends together once the upstream upgrades.
  if is_websocket_upgrade(&req) {
    return proxy_websocket(req, &upstream).await;
  }

  match rebuild_uri(&req, &upstream) {
    Some(new_uri) => {
      *req.uri_mut() = new_uri;
      strip_hop_headers(req.headers_mut());
      // Inject X-Forwarded-* headers.
      req.headers_mut().insert(
        "x-forwarded-host",
        header::HeaderValue::from_str(&host).unwrap_or_else(|_| header::HeaderValue::from_static("")),
      );
      req.headers_mut().insert(
        "x-forwarded-proto",
        match cfg.mode {
          ProxyMode::Http => header::HeaderValue::from_static("http"),
          ProxyMode::Https => header::HeaderValue::from_static("https"),
        },
      );
      let (parts, body) = req.into_parts();
      let body_boxed: ProxyBody = body.map_err(BoxError::from).boxed();
      let new_req = Request::from_parts(parts, body_boxed);
      match client.request(new_req).await {
        Ok(resp) => {
          let (parts, body) = resp.into_parts();
          let body_boxed: ProxyBody = body.map_err(BoxError::from).boxed();
          Response::from_parts(parts, body_boxed)
        }
        Err(e) => {
          tracing::debug!("proxy: upstream {upstream} error: {e}");
          error_page(StatusCode::BAD_GATEWAY, &format!("Upstream {upstream}: {e}"))
        }
      }
    }
    None => error_page(StatusCode::BAD_REQUEST, "Cannot rewrite request URI"),
  }
}

fn extract_host(req: &Request<Incoming>) -> Option<String> {
  if let Some(authority) = req.uri().authority() {
    return Some(authority.host().to_ascii_lowercase());
  }
  req.headers().get(header::HOST).and_then(|v| v.to_str().ok()).map(|s| {
    let trimmed = s.split(':').next().unwrap_or(s);
    trimmed.to_ascii_lowercase()
  })
}

fn strip_suffix(host: &str, suffix: &str) -> Option<String> {
  let suffix_no_dot = suffix.trim_end_matches('.');
  let trimmed = host.trim_end_matches('.');
  if !trimmed.ends_with(suffix_no_dot) {
    return None;
  }
  let head = &trimmed[..trimmed.len() - suffix_no_dot.len()];
  let head = head.trim_end_matches('.');
  if head.is_empty() {
    return None;
  }
  Some(head.to_ascii_lowercase())
}

fn upstream_authority(backend: &Backend) -> std::result::Result<String, &'static str> {
  match backend {
    Backend::Bridge { ip, port } => Ok(format!("{ip}:{port}")),
    Backend::HostPort { port } => Ok(format!("127.0.0.1:{port}")),
    Backend::Unreachable { reason } => Err(reason),
  }
}

fn rebuild_uri(req: &Request<Incoming>, authority: &str) -> Option<Uri> {
  let path_and_query = req
    .uri()
    .path_and_query()
    .map_or_else(|| "/".to_string(), ToString::to_string);
  format!("http://{authority}{path_and_query}").parse().ok()
}

fn strip_hop_headers(headers: &mut header::HeaderMap) {
  // RFC 7230 §6.1 + Connection-listed.
  static HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
  ];
  // First gather names listed in Connection header (must drop them too).
  let mut conn_listed: Vec<String> = Vec::new();
  if let Some(v) = headers.get(header::CONNECTION).and_then(|v| v.to_str().ok()) {
    for tok in v.split(',') {
      let tok = tok.trim();
      if !tok.is_empty() {
        conn_listed.push(tok.to_ascii_lowercase());
      }
    }
  }
  for name in HOP {
    headers.remove(*name);
  }
  for name in conn_listed {
    headers.remove(name);
  }
}

fn is_websocket_upgrade(req: &Request<Incoming>) -> bool {
  let upgrade = req
    .headers()
    .get(header::UPGRADE)
    .and_then(|v| v.to_str().ok())
    .unwrap_or_default();
  let connection = req
    .headers()
    .get(header::CONNECTION)
    .and_then(|v| v.to_str().ok())
    .unwrap_or_default();
  upgrade.eq_ignore_ascii_case("websocket") && connection.to_ascii_lowercase().contains("upgrade")
}

async fn proxy_websocket(mut req: Request<Incoming>, upstream: &str) -> Response<ProxyBody> {
  let upstream_authority: String = upstream.to_string();
  let stream = match tokio::net::TcpStream::connect(&upstream_authority).await {
    Ok(s) => s,
    Err(e) => return error_page(StatusCode::BAD_GATEWAY, &format!("Cannot connect upstream: {e}")),
  };
  let io = TokioIo::new(stream);
  let (mut sender, conn) = match hyper::client::conn::http1::handshake::<_, ProxyBody>(io).await {
    Ok(h) => h,
    Err(e) => return error_page(StatusCode::BAD_GATEWAY, &format!("Upstream handshake failed: {e}")),
  };
  tokio::spawn(async move {
    if let Err(e) = conn.with_upgrades().await {
      tracing::debug!("proxy ws: upstream conn error: {e}");
    }
  });

  let upstream_req = build_ws_upstream_request(&req, &upstream_authority);
  let client_upgrade: OnUpgrade = hyper::upgrade::on(&mut req);

  let upstream_resp = match sender.send_request(upstream_req).await {
    Ok(r) => r,
    Err(e) => return error_page(StatusCode::BAD_GATEWAY, &format!("Upstream send failed: {e}")),
  };
  if upstream_resp.status() != StatusCode::SWITCHING_PROTOCOLS {
    let (parts, body) = upstream_resp.into_parts();
    let body_boxed: ProxyBody = body.map_err(BoxError::from).boxed();
    return Response::from_parts(parts, body_boxed);
  }

  let mut switch = Response::new(empty());
  *switch.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
  for (k, v) in upstream_resp.headers() {
    switch.headers_mut().insert(k, v.clone());
  }

  let upstream_upgrade = hyper::upgrade::on(upstream_resp);
  tokio::spawn(async move {
    let (client_io, server_io) = match tokio::join!(client_upgrade, upstream_upgrade) {
      (Ok(c), Ok(s)) => (TokioIo::new(c), TokioIo::new(s)),
      (Err(e), _) | (_, Err(e)) => {
        tracing::debug!("proxy ws: upgrade failed: {e}");
        return;
      }
    };
    let mut c = client_io;
    let mut s = server_io;
    if let Err(e) = tokio::io::copy_bidirectional(&mut c, &mut s).await {
      tracing::debug!("proxy ws: copy_bidirectional ended: {e}");
    }
    let _ = c.shutdown().await;
    let _ = s.shutdown().await;
  });

  switch
}

fn build_ws_upstream_request(req: &Request<Incoming>, authority: &str) -> Request<ProxyBody> {
  let uri: Uri = format!(
    "http://{authority}{}",
    req
      .uri()
      .path_and_query()
      .map_or_else(|| "/".to_string(), ToString::to_string)
  )
  .parse()
  .unwrap_or_else(|_| Uri::from_static("/"));

  let mut builder = Request::builder().method(req.method()).uri(uri).version(req.version());
  for (k, v) in req.headers() {
    let lower = k.as_str().to_ascii_lowercase();
    if matches!(lower.as_str(), "host" | "content-length") {
      continue;
    }
    builder = builder.header(k, v);
  }
  builder = builder.header(header::HOST, authority);
  builder.body(empty()).unwrap_or_else(|_| {
    let mut r = Request::new(empty());
    *r.uri_mut() = Uri::from_static("/");
    r
  })
}

fn error_page(status: StatusCode, message: &str) -> Response<ProxyBody> {
  let body = format!(
    "<!doctype html><html><head><meta charset=utf-8><title>{} {}</title></head>\
<body style='font-family:system-ui;padding:2em;max-width:40em;margin:auto'>\
<h1>{} {}</h1><p>{}</p>\
<hr><p style='color:#666;font-size:0.85em'>dockside reverse proxy</p>\
</body></html>",
    status.as_u16(),
    status.canonical_reason().unwrap_or(""),
    status.as_u16(),
    status.canonical_reason().unwrap_or(""),
    html_escape(message),
  );
  let body_full: Full<Bytes> = Full::new(Bytes::from(body));
  let boxed: ProxyBody = body_full.map_err(|never| match never {}).boxed();
  Response::builder()
    .status(status)
    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
    .body(boxed)
    .unwrap_or_else(|_| Response::new(empty()))
}

fn redirect_to_https(req: &Request<Incoming>, host: &str, https_port: u16) -> Response<ProxyBody> {
  let path_and_query = req
    .uri()
    .path_and_query()
    .map_or_else(|| "/".to_string(), ToString::to_string);
  let target = if https_port == 443 {
    format!("https://{host}{path_and_query}")
  } else {
    format!("https://{host}:{https_port}{path_and_query}")
  };
  Response::builder()
    .status(StatusCode::TEMPORARY_REDIRECT)
    .header(header::LOCATION, target)
    .body(empty())
    .unwrap_or_else(|_| Response::new(empty()))
}

fn empty() -> ProxyBody {
  Full::<Bytes>::new(Bytes::new()).map_err(|never| match never {}).boxed()
}

fn html_escape(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::services::dns::Route;

  fn noop_route(name: &str, backend: Backend) -> Route {
    Route {
      container_id: format!("c-{name}"),
      primary: name.to_string(),
      aliases: vec![],
      backend,
      https_only: false,
      http_only: false,
    }
  }

  #[test]
  fn strip_suffix_handles_trailing_dot() {
    assert_eq!(
      strip_suffix("nginx.dockside.test", "dockside.test."),
      Some("nginx".into())
    );
    assert_eq!(
      strip_suffix("nginx.dockside.test.", "dockside.test."),
      Some("nginx".into())
    );
  }

  #[test]
  fn strip_suffix_rejects_apex() {
    assert_eq!(strip_suffix("dockside.test", "dockside.test."), None);
  }

  #[test]
  fn upstream_authority_for_each_backend() {
    let r = noop_route(
      "x",
      Backend::Bridge {
        ip: "172.17.0.2".parse().unwrap(),
        port: 80,
      },
    );
    assert_eq!(upstream_authority(&r.backend), Ok("172.17.0.2:80".to_string()));

    let r2 = noop_route("y", Backend::HostPort { port: 8080 });
    assert_eq!(upstream_authority(&r2.backend), Ok("127.0.0.1:8080".to_string()));

    let r3 = noop_route("z", Backend::Unreachable { reason: "no port" });
    assert_eq!(upstream_authority(&r3.backend), Err("no port"));
  }
}
