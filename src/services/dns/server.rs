//! Tiny authoritative DNS server. Answers `<name>.<suffix>` with `127.0.0.1`
//! (the reverse proxy is what actually routes to the container) and returns
//! NXDOMAIN for everything else, including the apex.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use hickory_proto::op::{Message, OpCode, ResponseCode};
use hickory_proto::rr::rdata::{A, SOA};
use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::oneshot;

use super::route_map::SharedRouteMap;

pub struct DnsServerHandle {
  shutdown: Option<oneshot::Sender<()>>,
  pub bound_port: u16,
}

impl Drop for DnsServerHandle {
  fn drop(&mut self) {
    if let Some(tx) = self.shutdown.take() {
      let _ = tx.send(());
    }
  }
}

#[derive(Clone)]
pub struct DocksideDnsHandler {
  /// Lower-cased FQDN suffix served by this resolver, no leading dot,
  /// trailing dot included for `Name` comparisons (e.g. `dockside.test.`).
  suffix: Arc<str>,
  routes: SharedRouteMap,
}

impl DocksideDnsHandler {
  pub fn new(suffix: &str, routes: SharedRouteMap) -> Self {
    let mut s = suffix.trim().trim_matches('.').to_ascii_lowercase();
    s.push('.');
    Self {
      suffix: Arc::from(s.as_str()),
      routes,
    }
  }

  pub(crate) fn extract_label(&self, fqdn: &Name) -> Option<String> {
    let lower = fqdn.to_lowercase().to_ascii();
    let suffix: &str = &self.suffix;
    if !lower.ends_with(suffix) {
      return None;
    }
    let head = &lower[..lower.len() - suffix.len()];
    let head = head.trim_end_matches('.');
    if head.is_empty() {
      return None;
    }
    Some(head.to_string())
  }

  fn zone_name(&self) -> Name {
    Name::from_ascii(self.suffix.as_ref()).unwrap_or_else(|_| Name::root())
  }

  fn build_soa(&self) -> Record {
    let zone = self.zone_name();
    let mname = Name::from_ascii(format!("ns1.{}", self.suffix)).unwrap_or_else(|_| zone.clone());
    let rname = Name::from_ascii(format!("hostmaster.{}", self.suffix)).unwrap_or_else(|_| zone.clone());
    let soa = SOA::new(mname, rname, 1, 60, 30, 86400, 30);
    Record::from_rdata(zone, 30, RData::SOA(soa))
  }

  fn respond(&self, query: &Message) -> Message {
    let mut response = Message::response(query.metadata.id, OpCode::Query);
    response.metadata.authoritative = true;
    response.metadata.recursion_desired = query.metadata.recursion_desired;

    let mut answers: Vec<Record> = Vec::new();
    let mut nxdomain = true;

    for q in &query.queries {
      response.add_query(q.clone());
      if q.query_class() != DNSClass::IN {
        continue;
      }
      let qname = q.name().clone();
      let Some(label) = self.extract_label(&qname) else {
        continue;
      };
      let known = {
        let routes = self.routes.read();
        routes.lookup(&label).is_some()
      };
      if !known {
        continue;
      }
      match q.query_type() {
        RecordType::A => {
          answers.push(Record::from_rdata(qname, 30, RData::A(A(Ipv4Addr::LOCALHOST))));
          nxdomain = false;
        }
        RecordType::AAAA => {
          // Known label, no AAAA — empty answer (NoData), not NXDOMAIN.
          nxdomain = false;
        }
        _ => {
          nxdomain = false;
        }
      }
    }

    if answers.is_empty() {
      // No answers — either NXDOMAIN (off-zone or unknown label) or NoData
      // (known label, unsupported type).
      response.metadata.response_code = if nxdomain {
        ResponseCode::NXDomain
      } else {
        ResponseCode::NoError
      };
      response.add_authority(self.build_soa());
    } else {
      response.add_answers(answers);
      response.metadata.response_code = ResponseCode::NoError;
    }
    response
  }
}

pub async fn spawn(handler: DocksideDnsHandler, port: u16) -> Result<DnsServerHandle> {
  let udp = UdpSocket::bind(("127.0.0.1", port))
    .await
    .with_context(|| format!("bind UDP 127.0.0.1:{port}"))?;
  let tcp = TcpListener::bind(("127.0.0.1", port))
    .await
    .with_context(|| format!("bind TCP 127.0.0.1:{port}"))?;
  let bound_port = udp.local_addr()?.port();

  let (tx, rx) = oneshot::channel::<()>();
  let (udp_shutdown_tx, udp_shutdown_rx) = oneshot::channel::<()>();
  let (tcp_shutdown_tx, tcp_shutdown_rx) = oneshot::channel::<()>();

  // Fan-out the single shutdown into UDP + TCP shutdown signals.
  tokio::spawn(async move {
    let _ = rx.await;
    let _ = udp_shutdown_tx.send(());
    let _ = tcp_shutdown_tx.send(());
  });

  let udp_handler = handler.clone();
  let mut udp_shutdown = udp_shutdown_rx;
  tokio::spawn(async move {
    let mut buf = vec![0u8; 4096];
    loop {
      tokio::select! {
        _ = &mut udp_shutdown => break,
        result = udp.recv_from(&mut buf) => {
          match result {
            Ok((len, peer)) => {
              if let Some(reply) = decode_and_respond(&udp_handler, &buf[..len])
                && let Err(e) = udp.send_to(&reply, peer).await {
                tracing::debug!("dns udp: send_to error: {e}");
              }
            }
            Err(e) => {
              tracing::warn!("dns udp: recv_from error: {e}");
              tokio::time::sleep(Duration::from_millis(100)).await;
            }
          }
        }
      }
    }
  });

  let tcp_handler = handler;
  let mut tcp_shutdown = tcp_shutdown_rx;
  tokio::spawn(async move {
    loop {
      tokio::select! {
        _ = &mut tcp_shutdown => break,
        accept = tcp.accept() => {
          match accept {
            Ok((mut stream, _peer)) => {
              let h = tcp_handler.clone();
              tokio::spawn(async move {
                let mut len_buf = [0u8; 2];
                if stream.read_exact(&mut len_buf).await.is_err() {
                  return;
                }
                let len = u16::from_be_bytes(len_buf) as usize;
                let mut msg = vec![0u8; len];
                if stream.read_exact(&mut msg).await.is_err() {
                  return;
                }
                if let Some(reply) = decode_and_respond(&h, &msg) {
                  let mut out = Vec::with_capacity(2 + reply.len());
                  out.extend_from_slice(&u16::try_from(reply.len()).unwrap_or(0).to_be_bytes());
                  out.extend_from_slice(&reply);
                  let _ = stream.write_all(&out).await;
                }
              });
            }
            Err(e) => {
              tracing::warn!("dns tcp: accept error: {e}");
              tokio::time::sleep(Duration::from_millis(100)).await;
            }
          }
        }
      }
    }
  });

  Ok(DnsServerHandle {
    shutdown: Some(tx),
    bound_port,
  })
}

fn decode_and_respond(handler: &DocksideDnsHandler, bytes: &[u8]) -> Option<Vec<u8>> {
  let query = match Message::from_vec(bytes) {
    Ok(m) => m,
    Err(e) => {
      tracing::debug!("dns: failed to decode query: {e}");
      return None;
    }
  };
  let response = handler.respond(&query);
  match response.to_vec() {
    Ok(bytes) => Some(bytes),
    Err(e) => {
      tracing::debug!("dns: failed to encode response: {e}");
      None
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::services::dns::route_map::{Backend, Route, new_shared};
  use hickory_proto::op::Query;

  #[test]
  fn extract_label_strips_suffix() {
    let h = DocksideDnsHandler::new("dockside.test", new_shared());
    let n: Name = "nginx.dockside.test.".parse().unwrap();
    assert_eq!(h.extract_label(&n), Some("nginx".to_string()));
  }

  #[test]
  fn extract_label_returns_none_for_apex() {
    let h = DocksideDnsHandler::new("dockside.test", new_shared());
    let n: Name = "dockside.test.".parse().unwrap();
    assert_eq!(h.extract_label(&n), None);
  }

  #[test]
  fn extract_label_returns_none_off_zone() {
    let h = DocksideDnsHandler::new("dockside.test", new_shared());
    let n: Name = "example.com.".parse().unwrap();
    assert_eq!(h.extract_label(&n), None);
  }

  fn dummy_route(name: &str) -> Route {
    Route {
      container_id: format!("c-{name}"),
      primary: name.to_string(),
      aliases: vec![],
      backend: Backend::HostPort { port: 8080 },
      https_only: false,
      http_only: false,
    }
  }

  #[test]
  fn known_label_resolves_to_localhost() {
    let routes = new_shared();
    routes.write().upsert(&dummy_route("nginx"));
    let h = DocksideDnsHandler::new("dockside.test", routes);
    let mut q = Message::query();
    q.metadata.id = 42;
    q.add_query(Query::query("nginx.dockside.test.".parse().unwrap(), RecordType::A));
    let r = h.respond(&q);
    assert_eq!(r.metadata.response_code, ResponseCode::NoError);
    assert_eq!(r.answers.len(), 1);
    match &r.answers[0].data {
      RData::A(a) => assert_eq!(a.0, Ipv4Addr::LOCALHOST),
      other => panic!("expected A record, got {other:?}"),
    }
  }

  #[test]
  fn unknown_label_in_zone_is_nxdomain() {
    let routes = new_shared();
    let h = DocksideDnsHandler::new("dockside.test", routes);
    let mut q = Message::query();
    q.metadata.id = 7;
    q.add_query(Query::query("missing.dockside.test.".parse().unwrap(), RecordType::A));
    let r = h.respond(&q);
    assert_eq!(r.metadata.response_code, ResponseCode::NXDomain);
    assert_eq!(r.answers.len(), 0);
    assert!(r.authorities.iter().any(|rec| matches!(&rec.data, RData::SOA(_))));
  }

  #[test]
  fn off_zone_query_is_nxdomain() {
    let routes = new_shared();
    let h = DocksideDnsHandler::new("dockside.test", routes);
    let mut q = Message::query();
    q.metadata.id = 9;
    q.add_query(Query::query("example.com.".parse().unwrap(), RecordType::A));
    let r = h.respond(&q);
    assert_eq!(r.metadata.response_code, ResponseCode::NXDomain);
    assert_eq!(r.answers.len(), 0);
  }
}
