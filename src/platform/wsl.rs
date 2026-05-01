//! Windows-only WSL2 detection and Docker connection utilities
//!
//! This module provides functionality for detecting WSL2 distributions and
//! connecting to Docker running inside them from native Windows applications.

#![cfg(target_os = "windows")]

use std::process::Command;

use anyhow::{Result, anyhow};

/// Information about a WSL2 distribution
#[derive(Debug, Clone)]
pub struct WslDistro {
  /// Distribution name (e.g., "Ubuntu", "Debian")
  pub name: String,
  /// WSL version (1 or 2)
  pub version: u8,
  /// Whether this is the default distribution
  pub is_default: bool,
  /// Docker TCP port if Docker is configured and running
  pub docker_port: Option<u16>,
}

/// List all installed WSL distributions
///
/// # Errors
///
/// Returns an error if `wsl.exe` fails to execute or output cannot be parsed.
pub fn list_distros() -> Result<Vec<WslDistro>> {
  let output = Command::new("wsl.exe")
    .args(["-l", "-v"])
    .output()
    .map_err(|e| anyhow!("Failed to execute wsl.exe: {e}"))?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(anyhow!("wsl.exe failed: {stderr}"));
  }

  let stdout = String::from_utf8_lossy(&output.stdout);
  parse_wsl_list(&stdout)
}

/// Parse the output of `wsl.exe -l -v`
fn parse_wsl_list(output: &str) -> Result<Vec<WslDistro>> {
  let mut distros = Vec::new();

  // Skip the header line and parse each distro line
  // Format: "  NAME                   STATE           VERSION"
  // or    "* Ubuntu                  Running         2"
  for line in output.lines().skip(1) {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }

    // Remove null bytes that wsl.exe sometimes includes
    let line: String = line.chars().filter(|c| *c != '\0').collect();
    let line = line.trim();

    if line.is_empty() {
      continue;
    }

    let is_default = line.starts_with('*');
    let line = line.trim_start_matches('*').trim();

    // Split by whitespace and extract name and version
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
      let name = parts[0].to_string();
      // Version is typically the last column
      if let Ok(version) = parts.last().unwrap_or(&"0").parse::<u8>() {
        distros.push(WslDistro {
          name,
          version,
          is_default,
          docker_port: None,
        });
      }
    }
  }

  // For each WSL2 distro, check if Docker is available
  for distro in &mut distros {
    if distro.version == 2 {
      distro.docker_port = check_docker_in_distro(&distro.name);
    }
  }

  Ok(distros)
}

/// Check if Docker is running in a specific WSL2 distro and return the port
fn check_docker_in_distro(distro: &str) -> Option<u16> {
  // First check if docker command exists
  let which_output = Command::new("wsl.exe")
    .args(["-d", distro, "-e", "which", "docker"])
    .output()
    .ok()?;

  if !which_output.status.success() {
    return None;
  }

  // Check if Docker daemon is running by trying docker info
  let info_output = Command::new("wsl.exe")
    .args(["-d", distro, "-e", "docker", "info"])
    .output()
    .ok()?;

  if !info_output.status.success() {
    // Docker installed but not running
    return None;
  }

  // Check if Docker is configured to listen on TCP
  // Look for the configured port in daemon.json or check if port is open
  if let Some(port) = detect_docker_tcp_port(distro) {
    return Some(port);
  }

  // Default port if Docker is running but TCP config unknown
  // User may need to configure TCP listening
  Some(super::DockerRuntime::DEFAULT_DOCKER_TCP_PORT)
}

/// Detect the TCP port Docker is listening on in a WSL2 distro
fn detect_docker_tcp_port(distro: &str) -> Option<u16> {
  // Try to read daemon.json to find configured hosts
  let cat_output = Command::new("wsl.exe")
    .args(["-d", distro, "-e", "cat", "/etc/docker/daemon.json"])
    .output()
    .ok()?;

  if cat_output.status.success() {
    let content = String::from_utf8_lossy(&cat_output.stdout);
    // Look for tcp://... in hosts configuration
    if let Some(port) = parse_docker_tcp_port(&content) {
      return Some(port);
    }
  }

  // Check if default port is listening
  let netstat_output = Command::new("wsl.exe")
    .args(["-d", distro, "-e", "sh", "-c", "ss -tlnp | grep :2375"])
    .output()
    .ok()?;

  if netstat_output.status.success() && !netstat_output.stdout.is_empty() {
    return Some(super::DockerRuntime::DEFAULT_DOCKER_TCP_PORT);
  }

  // Check TLS port
  let tls_port = super::DockerRuntime::DEFAULT_DOCKER_TLS_PORT;
  let netstat_tls = Command::new("wsl.exe")
    .args(["-d", distro, "-e", "sh", "-c", &format!("ss -tlnp | grep :{tls_port}")])
    .output()
    .ok()?;

  if netstat_tls.status.success() && !netstat_tls.stdout.is_empty() {
    return Some(tls_port);
  }

  None
}

/// Parse TCP port from Docker daemon.json content
fn parse_docker_tcp_port(content: &str) -> Option<u16> {
  // Look for patterns like "tcp://0.0.0.0:2375" or "tcp://127.0.0.1:2375"
  for line in content.lines() {
    if line.contains("tcp://") {
      // Extract port number after the last colon
      if let Some(tcp_start) = line.find("tcp://") {
        let after_tcp = &line[tcp_start..];
        if let Some(port_start) = after_tcp.rfind(':') {
          let port_str: String = after_tcp[port_start + 1..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
          if let Ok(port) = port_str.parse::<u16>() {
            return Some(port);
          }
        }
      }
    }
  }
  None
}

/// Check if Docker is available in any WSL2 distro
///
/// Returns the first distro with Docker running, if any.
pub fn find_docker_distro() -> Option<WslDistro> {
  list_distros()
    .ok()?
    .into_iter()
    .find(|d| d.version == 2 && d.docker_port.is_some())
}

/// Spawn a TCP relay for Docker socket in WSL2
///
/// This creates a socat relay that forwards TCP connections to the Docker socket.
/// Useful when Docker isn't configured to listen on TCP directly.
///
/// # Arguments
///
/// * `distro` - WSL2 distribution name
/// * `port` - TCP port to listen on
///
/// # Errors
///
/// Returns an error if the relay process fails to spawn.
pub fn spawn_docker_relay(distro: &str, port: u16) -> Result<std::process::Child> {
  // Check if socat is installed
  let socat_check = Command::new("wsl.exe")
    .args(["-d", distro, "-e", "which", "socat"])
    .output()?;

  if !socat_check.status.success() {
    return Err(anyhow!(
      "socat is not installed in {distro}. Install it with: wsl -d {distro} -e sudo apt install socat"
    ));
  }

  // Spawn socat relay in background
  let child = Command::new("wsl.exe")
    .args([
      "-d",
      distro,
      "-e",
      "socat",
      &format!("TCP-LISTEN:{port},fork,bind=0.0.0.0"),
      "UNIX-CONNECT:/var/run/docker.sock",
    ])
    .spawn()
    .map_err(|e| anyhow!("Failed to spawn socat relay: {e}"))?;

  Ok(child)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_wsl_list_empty() {
    let result = parse_wsl_list("");
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
  }

  #[test]
  fn test_parse_wsl_list_with_distros() {
    let output = r#"  NAME                   STATE           VERSION
* Ubuntu                  Running         2
  Debian                  Stopped         2
  Alpine                  Running         1
"#;
    let result = parse_wsl_list(output);
    assert!(result.is_ok());
    let distros = result.unwrap();
    assert_eq!(distros.len(), 3);

    assert_eq!(distros[0].name, "Ubuntu");
    assert_eq!(distros[0].version, 2);
    assert!(distros[0].is_default);

    assert_eq!(distros[1].name, "Debian");
    assert_eq!(distros[1].version, 2);
    assert!(!distros[1].is_default);

    assert_eq!(distros[2].name, "Alpine");
    assert_eq!(distros[2].version, 1);
    assert!(!distros[2].is_default);
  }

  #[test]
  fn test_parse_docker_tcp_port() {
    let content = r#"
{
  "hosts": ["unix:///var/run/docker.sock", "tcp://0.0.0.0:2375"]
}
"#;
    assert_eq!(parse_docker_tcp_port(content), Some(2375));

    let content_tls = r#"
{
  "hosts": ["tcp://127.0.0.1:2376"]
}
"#;
    assert_eq!(parse_docker_tcp_port(content_tls), Some(2376));

    let content_no_tcp = r#"
{
  "hosts": ["unix:///var/run/docker.sock"]
}
"#;
    assert_eq!(parse_docker_tcp_port(content_no_tcp), None);
  }
}
