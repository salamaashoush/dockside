//! Shared in-memory map from container hostname to backend address.
//!
//! Owned by the DNS server (which only needs the hostname → 127.0.0.1 fact)
//! and the reverse proxy (which needs the actual backend tuple). Both share
//! the same `Arc<RwLock<RouteMap>>` so a single watcher task can mutate
//! once and both readers see fresh state.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use parking_lot::RwLock;

/// Where the reverse proxy should forward `<name>.<suffix>`.
#[derive(Debug, Clone)]
pub enum Backend {
  /// Reachable directly at `ip:port` from the host. Linux native, `OrbStack`
  /// with bridge access, Colima with `--network-address`.
  Bridge { ip: IpAddr, port: u16 },
  /// Reachable via published port on host loopback. Docker Desktop on
  /// `macOS`/Windows, Colima default.
  HostPort { port: u16 },
  /// No reachable target — proxy returns 502 with explanation.
  Unreachable { reason: &'static str },
}

#[derive(Debug, Clone)]
pub struct Route {
  pub container_id: String,
  /// Lowercased canonical name used as map key (also one of `aliases`).
  pub primary: String,
  pub aliases: Vec<String>,
  pub backend: Backend,
  /// `dockside.https=true` label — proxy refuses plain HTTP.
  pub https_only: bool,
  /// `dockside.http_only=true` label — proxy never redirects to HTTPS.
  pub http_only: bool,
}

#[derive(Debug, Default)]
pub struct RouteMap {
  /// Hostname (without suffix) → route. A single Route may appear under
  /// multiple keys (primary name + aliases).
  by_name: HashMap<String, Route>,
}

impl RouteMap {
  pub fn new() -> Self {
    Self {
      by_name: HashMap::new(),
    }
  }

  /// Replace all routes from a full container snapshot. Called on initial
  /// seed; partial updates use `upsert` / `remove` instead.
  pub fn replace(&mut self, routes: &[Route]) {
    self.by_name.clear();
    for route in routes {
      self.insert_route(route);
    }
  }

  /// Add or update a single container's routes. Removes any existing entries
  /// that referenced the same `container_id` first so renames + alias
  /// changes do not leave stale keys. Crate-visible so the proxy + DNS
  /// server tests can drive it without going through the watcher.
  pub(crate) fn upsert(&mut self, route: &Route) {
    self.remove_by_id(&route.container_id);
    self.insert_route(route);
  }

  pub(crate) fn remove_by_id(&mut self, container_id: &str) {
    self.by_name.retain(|_, route| route.container_id != container_id);
  }

  /// Lookup by hostname (lowercase, no suffix). Returns the route if any
  /// alias of any container matches.
  pub fn lookup(&self, name: &str) -> Option<&Route> {
    self.by_name.get(&name.to_lowercase())
  }

  /// Number of distinct containers currently routed (deduplicated by
  /// `container_id`). Surfaced on the Settings/Dashboard tile.
  pub fn distinct_container_count(&self) -> usize {
    let mut ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for route in self.by_name.values() {
      ids.insert(route.container_id.as_str());
    }
    ids.len()
  }

  /// Primary hostname for a given container id, or `None` if the container
  /// is not currently routed. Surfaced on container row + detail view as a
  /// clickable URL.
  pub fn primary_for_container(&self, container_id: &str) -> Option<String> {
    self
      .by_name
      .values()
      .find(|r| r.container_id == container_id)
      .map(|r| r.primary.clone())
  }

  /// Snapshot for the Settings UI: `(hostname, backend description)` pairs,
  /// one per registered hostname (primary + aliases). Sorted by hostname.
  pub fn route_summaries(&self) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = self
      .by_name
      .iter()
      .map(|(name, route)| {
        let target = match &route.backend {
          Backend::Bridge { ip, port } => format!("{ip}:{port} (bridge)"),
          Backend::HostPort { port } => format!("127.0.0.1:{port} (host port)"),
          Backend::Unreachable { reason } => format!("unreachable — {reason}"),
        };
        (name.clone(), target)
      })
      .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
  }

  fn insert_route(&mut self, route: &Route) {
    let primary = route.primary.clone();
    self.by_name.insert(primary, route.clone());
    for alias in &route.aliases {
      let key = alias.to_lowercase();
      self.by_name.insert(key, route.clone());
    }
  }
}

/// Cheap shared handle for the watcher / DNS server / proxy.
pub type SharedRouteMap = Arc<RwLock<RouteMap>>;

pub fn new_shared() -> SharedRouteMap {
  Arc::new(RwLock::new(RouteMap::new()))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::net::Ipv4Addr;

  fn route(id: &str, primary: &str, aliases: &[&str]) -> Route {
    Route {
      container_id: id.to_string(),
      primary: primary.to_string(),
      aliases: aliases.iter().map(|s| (*s).to_string()).collect(),
      backend: Backend::Bridge {
        ip: IpAddr::V4(Ipv4Addr::new(172, 17, 0, 2)),
        port: 80,
      },
      https_only: false,
      http_only: false,
    }
  }

  #[test]
  fn lookup_matches_primary_and_aliases() {
    let mut m = RouteMap::new();
    m.upsert(&route("c1", "nginx", &["web", "frontend"]));
    assert!(m.lookup("nginx").is_some());
    assert!(m.lookup("web").is_some());
    assert!(m.lookup("frontend").is_some());
    assert!(m.lookup("missing").is_none());
  }

  #[test]
  fn lookup_is_case_insensitive() {
    let mut m = RouteMap::new();
    m.upsert(&route("c1", "nginx", &[]));
    assert!(m.lookup("NGINX").is_some());
    assert!(m.lookup("Nginx").is_some());
  }

  #[test]
  fn upsert_removes_old_aliases_for_same_container() {
    let mut m = RouteMap::new();
    m.upsert(&route("c1", "nginx", &["web"]));
    m.upsert(&route("c1", "nginx", &["api"]));
    assert!(m.lookup("nginx").is_some());
    assert!(m.lookup("api").is_some());
    assert!(m.lookup("web").is_none());
  }

  #[test]
  fn remove_by_id_clears_all_aliases() {
    let mut m = RouteMap::new();
    m.upsert(&route("c1", "nginx", &["web"]));
    m.upsert(&route("c2", "redis", &[]));
    m.remove_by_id("c1");
    assert!(m.lookup("nginx").is_none());
    assert!(m.lookup("web").is_none());
    assert!(m.lookup("redis").is_some());
  }

  #[test]
  fn distinct_container_count_dedupes_aliases() {
    let mut m = RouteMap::new();
    m.upsert(&route("c1", "nginx", &["web", "frontend"]));
    m.upsert(&route("c2", "redis", &[]));
    assert_eq!(m.distinct_container_count(), 2);
  }
}
