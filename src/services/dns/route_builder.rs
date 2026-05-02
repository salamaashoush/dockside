//! Translate `ContainerInfo` (from `docker ps`) plus runtime detection
//! into `Route` entries the DNS server + reverse proxy can use.

use std::net::IpAddr;

use crate::docker::ContainerInfo;

use super::route_map::{Backend, Route};

/// What kind of Docker daemon are we talking to? Determines whether bridge
/// IPs are routable from the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
  /// Linux native — bridge IPs reachable directly via `docker0`.
  LinuxNative,
  /// Docker Desktop / Colima default / any LinuxKit-style VM — bridge IPs
  /// not host-routable; must use published ports on `127.0.0.1`.
  VmBacked,
  /// Unknown — assume VM-backed (safer default; falls back to host ports).
  Unknown,
}

impl RuntimeKind {
  /// Detect from `bollard::system::version`-style output. We pass in the
  /// `OperatingSystem` field (e.g. "Docker Desktop", "Ubuntu 22.04 LTS")
  /// and the `KernelVersion` (e.g. "6.10.14-linuxkit").
  pub fn detect(operating_system: &str, kernel_version: &str) -> Self {
    let os_lc = operating_system.to_ascii_lowercase();
    let kernel_lc = kernel_version.to_ascii_lowercase();
    if os_lc.contains("docker desktop")
      || os_lc.contains("orbstack")
      || kernel_lc.contains("linuxkit")
      || kernel_lc.contains("colima")
    {
      // OrbStack actually has routable bridge IPs when its Network setting
      // is on, but we cannot detect that flag — treat as VM-backed unless
      // the user opts into bridge mode via the `dockside.backend=bridge`
      // label per container.
      return RuntimeKind::VmBacked;
    }
    if cfg!(target_os = "linux") {
      RuntimeKind::LinuxNative
    } else {
      RuntimeKind::Unknown
    }
  }
}

/// Build a Route from a `ContainerInfo`. Returns `None` for containers that
/// have opted out (`dockside.disable=true`) or have no resolvable backend.
pub fn route_from_container(c: &ContainerInfo, runtime: RuntimeKind) -> Option<Route> {
  if !c.state.is_running() && !c.state.is_paused() {
    return None;
  }
  if label_truthy(c, "dockside.disable") {
    return None;
  }

  let primary = sanitize_name(&c.name);
  if primary.is_empty() {
    return None;
  }

  let mut aliases: Vec<String> = Vec::new();
  // Compose-style alias: `<service>.<project>` so users get
  // `web.myapp.dockside.test` for free.
  if let (Some(project), Some(service)) = (
    c.labels.get("com.docker.compose.project"),
    c.labels.get("com.docker.compose.service"),
  ) {
    aliases.push(format!("{service}.{project}"));
    aliases.push(service.clone());
  }
  // User-provided extra aliases via `dockside.alias=foo,bar`.
  if let Some(raw) = c.labels.get("dockside.alias") {
    for piece in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
      aliases.push(piece.to_string());
    }
  }

  let backend = derive_backend(c, runtime)?;

  let only_https = label_truthy(c, "dockside.https");
  let only_http = label_truthy(c, "dockside.http_only");

  Some(Route {
    container_id: c.id.clone(),
    primary,
    aliases,
    backend,
    https_only: only_https,
    http_only: only_http,
  })
}

fn derive_backend(c: &ContainerInfo, runtime: RuntimeKind) -> Option<Backend> {
  let label_port = c.labels.get("dockside.port").and_then(|s| s.parse::<u16>().ok());
  let backend_override = c.labels.get("dockside.backend").map(String::as_str);

  if backend_override == Some("bridge") {
    let port = label_port.or_else(|| lowest_private_port(c))?;
    let ip = bridge_ip(c)?;
    return Some(Backend::Bridge { ip, port });
  }
  if backend_override == Some("host") {
    let port = label_port.or_else(|| lowest_public_port(c))?;
    return Some(Backend::HostPort { port });
  }

  match runtime {
    RuntimeKind::LinuxNative => {
      let port = label_port.or_else(|| lowest_private_port(c))?;
      bridge_ip(c).map_or(
        Some(Backend::Unreachable {
          reason: "container has no bridge IP",
        }),
        |ip| Some(Backend::Bridge { ip, port }),
      )
    }
    RuntimeKind::VmBacked | RuntimeKind::Unknown => {
      if let Some(public) = lowest_public_port(c) {
        Some(Backend::HostPort {
          port: label_port.unwrap_or(public),
        })
      } else {
        Some(Backend::Unreachable {
          reason: "no published port — Docker Desktop requires `docker run -p` to reach a container",
        })
      }
    }
  }
}

fn lowest_private_port(c: &ContainerInfo) -> Option<u16> {
  c.ports.iter().map(|p| p.private_port).min()
}

fn lowest_public_port(c: &ContainerInfo) -> Option<u16> {
  c.ports.iter().filter_map(|p| p.public_port).min()
}

fn bridge_ip(c: &ContainerInfo) -> Option<IpAddr> {
  c.bridge_ip
}

fn sanitize_name(name: &str) -> String {
  name.trim_start_matches('/').to_ascii_lowercase()
}

fn label_truthy(c: &ContainerInfo, key: &str) -> bool {
  matches!(
    c.labels.get(key).map(String::as_str),
    Some("1" | "true" | "True" | "yes" | "on")
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detect_linuxkit_is_vm_backed() {
    assert_eq!(
      RuntimeKind::detect("Docker Desktop", "6.10.14-linuxkit"),
      RuntimeKind::VmBacked
    );
  }

  #[test]
  fn detect_native_linux() {
    if cfg!(target_os = "linux") {
      assert_eq!(
        RuntimeKind::detect("Ubuntu 22.04 LTS", "6.8.0-50-generic"),
        RuntimeKind::LinuxNative
      );
    }
  }
}
