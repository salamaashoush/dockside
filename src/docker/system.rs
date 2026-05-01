//! Docker system information from the Docker daemon API

use anyhow::Result;
use bollard::models::SystemInfo;

use super::DockerClient;

/// Information about the Docker host system
#[derive(Debug, Clone)]
pub struct DockerHostInfo {
  /// Hostname of the Docker host
  pub name: String,
  /// Operating system name (e.g., "Arch Linux", "Ubuntu 22.04")
  pub os: String,
  /// Kernel version
  pub kernel: String,
  /// CPU architecture (e.g., "`x86_64`", "aarch64")
  pub arch: String,
  /// Number of CPUs
  pub cpus: u32,
  /// Total memory in bytes
  pub memory: u64,
  /// Docker version
  pub docker_version: String,
  /// Docker API version
  pub api_version: String,
  /// Storage driver (e.g., "overlay2")
  pub storage_driver: String,
  /// Docker root directory (e.g., "/var/lib/docker")
  pub docker_root: String,
  /// Total number of containers
  pub containers_total: u64,
  /// Number of running containers
  pub containers_running: u64,
  /// Number of paused containers
  pub containers_paused: u64,
  /// Number of stopped containers
  pub containers_stopped: u64,
  /// Total number of images
  pub images: u64,
  /// Container runtime (e.g., "containerd", "runc")
  pub runtime: String,
  /// Docker socket path
  pub docker_socket: String,
}

impl DockerHostInfo {
  /// Create `DockerHostInfo` from bollard's `SystemInfo`
  #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
  pub fn from_system_info(info: SystemInfo, socket_path: String) -> Self {
    // Extract API version from server_version if available, otherwise use a default
    let api_version = info.server_version.as_ref().map_or_else(
      || "Unknown".to_string(),
      |v| {
        // Docker server version like "27.0.3" corresponds to API version
        // but we don't have direct API version in SystemInfo
        // We'll use the server version as a fallback
        format!("1.{}", v.split('.').next().unwrap_or("46"))
      },
    );

    Self {
      name: info.name.unwrap_or_else(|| "Docker Host".to_string()),
      os: info.operating_system.unwrap_or_else(|| "Unknown".to_string()),
      kernel: info.kernel_version.unwrap_or_else(|| "Unknown".to_string()),
      arch: info.architecture.unwrap_or_else(|| "Unknown".to_string()),
      cpus: info.ncpu.unwrap_or(0) as u32,
      memory: info.mem_total.unwrap_or(0) as u64,
      docker_version: info.server_version.unwrap_or_else(|| "Unknown".to_string()),
      api_version,
      storage_driver: info.driver.unwrap_or_else(|| "Unknown".to_string()),
      docker_root: info.docker_root_dir.unwrap_or_else(|| "/var/lib/docker".to_string()),
      containers_total: info.containers.unwrap_or(0) as u64,
      containers_running: info.containers_running.unwrap_or(0) as u64,
      containers_paused: info.containers_paused.unwrap_or(0) as u64,
      containers_stopped: info.containers_stopped.unwrap_or(0) as u64,
      images: info.images.unwrap_or(0) as u64,
      runtime: info.default_runtime.unwrap_or_else(|| "runc".to_string()),
      docker_socket: socket_path,
    }
  }

  /// Get memory in gigabytes for display
  #[allow(clippy::cast_precision_loss)]
  pub fn memory_gb(&self) -> f64 {
    self.memory as f64 / (1024.0 * 1024.0 * 1024.0)
  }

  /// Get formatted memory string
  #[allow(clippy::cast_precision_loss)]
  pub fn display_memory(&self) -> String {
    let gb = self.memory_gb();
    if gb >= 1.0 {
      format!("{gb:.1} GB")
    } else {
      let mb = self.memory as f64 / (1024.0 * 1024.0);
      format!("{mb:.0} MB")
    }
  }
}

impl DockerClient {
  /// Get system information from the Docker daemon
  pub async fn get_system_info(&self) -> Result<DockerHostInfo> {
    let docker = self.client()?;
    let info = docker.info().await?;
    let socket_path = self.connection_string();
    Ok(DockerHostInfo::from_system_info(info, socket_path))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn create_test_host_info() -> DockerHostInfo {
    DockerHostInfo {
      name: "test-host".to_string(),
      os: "Arch Linux".to_string(),
      kernel: "6.18.6-arch1-1".to_string(),
      arch: "x86_64".to_string(),
      cpus: 16,
      memory: 32 * 1024 * 1024 * 1024, // 32 GB
      docker_version: "27.0.3".to_string(),
      api_version: "1.46".to_string(),
      storage_driver: "overlay2".to_string(),
      docker_root: "/var/lib/docker".to_string(),
      containers_total: 12,
      containers_running: 5,
      containers_paused: 0,
      containers_stopped: 7,
      images: 47,
      runtime: "runc".to_string(),
      docker_socket: "/var/run/docker.sock".to_string(),
    }
  }

  #[test]
  fn test_memory_gb() {
    let host = create_test_host_info();
    assert!((host.memory_gb() - 32.0).abs() < 0.01);
  }

  #[test]
  fn test_display_memory() {
    let host = create_test_host_info();
    assert_eq!(host.display_memory(), "32.0 GB");

    let small_host = DockerHostInfo {
      memory: 512 * 1024 * 1024, // 512 MB
      ..create_test_host_info()
    };
    assert_eq!(small_host.display_memory(), "512 MB");
  }

  #[test]
  fn test_from_system_info() {
    let info = SystemInfo {
      name: Some("my-host".to_string()),
      operating_system: Some("Ubuntu 22.04".to_string()),
      kernel_version: Some("5.15.0".to_string()),
      architecture: Some("aarch64".to_string()),
      ncpu: Some(8),
      mem_total: Some(16 * 1024 * 1024 * 1024),
      server_version: Some("24.0.0".to_string()),
      driver: Some("overlay2".to_string()),
      docker_root_dir: Some("/var/lib/docker".to_string()),
      containers: Some(10),
      containers_running: Some(3),
      containers_paused: Some(1),
      containers_stopped: Some(6),
      images: Some(25),
      default_runtime: Some("runc".to_string()),
      ..Default::default()
    };

    let host = DockerHostInfo::from_system_info(info, "/var/run/docker.sock".to_string());
    assert_eq!(host.name, "my-host");
    assert_eq!(host.os, "Ubuntu 22.04");
    assert_eq!(host.kernel, "5.15.0");
    assert_eq!(host.arch, "aarch64");
    assert_eq!(host.cpus, 8);
    assert_eq!(host.containers_total, 10);
    assert_eq!(host.containers_running, 3);
    assert_eq!(host.images, 25);
  }

  #[test]
  fn test_from_system_info_defaults() {
    let info = SystemInfo::default();
    let host = DockerHostInfo::from_system_info(info, "/var/run/docker.sock".to_string());
    assert_eq!(host.name, "Docker Host");
    assert_eq!(host.os, "Unknown");
    assert_eq!(host.cpus, 0);
    assert_eq!(host.docker_root, "/var/lib/docker");
  }
}
