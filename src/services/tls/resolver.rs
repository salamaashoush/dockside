//! `rustls::ResolvesServerCert` implementation that lazy-mints leaf certs
//! per SNI host out of the local CA. Caches issued certs in an LRU keyed
//! by hostname so repeated handshakes skip the keygen+sign cost.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rustls::crypto::CryptoProvider;
use rustls::crypto::ring::default_provider;
use rustls::pki_types::CertificateDer;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

use super::ca::LocalCa;

const CACHE_LIMIT: usize = 256;

#[derive(Debug)]
pub struct DocksideCertResolver {
  ca: Arc<LocalCa>,
  provider: Arc<CryptoProvider>,
  cache: Mutex<HashMap<String, Arc<CertifiedKey>>>,
  /// FIFO insertion order so we can evict the oldest entry when the LRU is full.
  insertion_order: Mutex<Vec<String>>,
  suffix: Arc<str>,
}

impl DocksideCertResolver {
  pub fn new(ca: Arc<LocalCa>, suffix: Arc<str>) -> Self {
    let provider = Arc::new(default_provider());
    Self {
      ca,
      provider,
      cache: Mutex::new(HashMap::new()),
      insertion_order: Mutex::new(Vec::new()),
      suffix,
    }
  }

  fn issue(&self, dns_name: &str) -> Result<Arc<CertifiedKey>> {
    let leaf = self.ca.issue_leaf(dns_name).context("issue leaf cert")?;
    let cert_der = parse_cert_chain(&leaf.cert_pem).context("parse leaf cert PEM")?;
    let key_der = parse_private_key(&leaf.key_pem).context("parse leaf key PEM")?;
    let certified = CertifiedKey::from_der(cert_der, key_der, &self.provider).context("build CertifiedKey")?;
    Ok(Arc::new(certified))
  }

  fn cached_or_issue(&self, dns_name: &str) -> Option<Arc<CertifiedKey>> {
    {
      let cache = self.cache.lock();
      if let Some(ck) = cache.get(dns_name) {
        return Some(ck.clone());
      }
    }
    match self.issue(dns_name) {
      Ok(ck) => {
        let mut cache = self.cache.lock();
        let mut order = self.insertion_order.lock();
        if cache.len() >= CACHE_LIMIT
          && let Some(oldest) = order.first().cloned()
        {
          cache.remove(&oldest);
          order.remove(0);
        }
        cache.insert(dns_name.to_string(), ck.clone());
        order.push(dns_name.to_string());
        Some(ck)
      }
      Err(e) => {
        tracing::warn!("tls: failed to issue leaf for {dns_name}: {e}");
        None
      }
    }
  }
}

impl ResolvesServerCert for DocksideCertResolver {
  fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
    let host = client_hello.server_name()?.to_ascii_lowercase();
    // Only mint inside our zone. Off-zone SNIs get None → handshake fails.
    let suffix_no_dot = self.suffix.trim_end_matches('.');
    if !host.ends_with(suffix_no_dot) {
      return None;
    }
    self.cached_or_issue(&host)
  }
}

fn parse_cert_chain(pem: &str) -> Result<Vec<CertificateDer<'static>>> {
  use rustls_pemfile::Item;
  let mut chain = Vec::new();
  for item in rustls_pemfile::read_all(&mut pem.as_bytes()).flatten() {
    if let Item::X509Certificate(der) = item {
      chain.push(der);
    }
  }
  if chain.is_empty() {
    anyhow::bail!("no certificates found in PEM");
  }
  Ok(chain)
}

fn parse_private_key(pem: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>> {
  use rustls_pemfile::Item;
  for item in rustls_pemfile::read_all(&mut pem.as_bytes()).flatten() {
    match item {
      Item::Pkcs8Key(k) => return Ok(k.into()),
      Item::Pkcs1Key(k) => return Ok(k.into()),
      Item::Sec1Key(k) => return Ok(k.into()),
      _ => {}
    }
  }
  anyhow::bail!("no private key found in PEM")
}
