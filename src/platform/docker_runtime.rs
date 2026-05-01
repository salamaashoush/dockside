//! Docker runtime abstraction for cross-platform support

use std::path::Path;

use super::Platform;

/// Docker runtime configuration for different platforms and setups
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockerRuntime {
  /// Colima VM on macOS or Linux
  Colima {
    /// Profile name (e.g., "default", "dev")
    profile: String,
  },
  /// Native Docker daemon with Unix socket
  NativeDocker {
    /// Path to the Unix socket
    socket_path: String,
  },
  /// Docker running in WSL2, accessed via TCP from Windows
  Wsl2Docker {
    /// WSL2 distro name (e.g., "Ubuntu", "Debian")
    distro: String,
    /// TCP port Docker is listening on
    port: u16,
  },
  /// Rootless Docker installation
  RootlessDocker {
    /// Path to the user's Docker socket
    socket_path: String,
  },
  /// Custom connection configuration
  Custom {
    /// Connection string (socket path or HTTP URL)
    connection_string: String,
  },
}

impl DockerRuntime {
  /// Default TCP port for Docker daemon
  pub const DEFAULT_DOCKER_TCP_PORT: u16 = 2375;
  /// Default TCP port for Docker daemon with TLS
  #[allow(dead_code)] // Used in WSL2 docker port detection (Windows-only)
  pub const DEFAULT_DOCKER_TLS_PORT: u16 = 2376;

  /// Get the connection string for this runtime
  #[must_use]
  pub fn connection_string(&self) -> String {
    match self {
      Self::Colima { profile } => {
        let home = dirs::home_dir().unwrap_or_default();
        format!("{}/.colima/{}/docker.sock", home.display(), profile)
      }
      Self::NativeDocker { socket_path } | Self::RootlessDocker { socket_path } => socket_path.clone(),
      Self::Wsl2Docker { distro: _, port } => {
        format!("http://localhost:{port}")
      }
      Self::Custom { connection_string } => connection_string.clone(),
    }
  }

  /// Check if this runtime uses TCP/HTTP connection (vs Unix socket)
  #[must_use]
  pub fn uses_tcp(&self) -> bool {
    match self {
      Self::Wsl2Docker { .. } => true,
      Self::Custom { connection_string } => {
        connection_string.starts_with("http://") || connection_string.starts_with("tcp://")
      }
      _ => false,
    }
  }

  /// Check if the runtime's socket/connection is available
  #[must_use]
  pub fn is_available(&self) -> bool {
    // TCP/HTTP runtimes can't be probed without an actual connection; defer to
    // the live connect attempt instead of pre-checking.
    if self.uses_tcp() {
      return true;
    }
    Path::new(&self.connection_string()).exists()
  }

  /// Get the display name for this runtime
  #[must_use]
  pub fn display_name(&self) -> String {
    match self {
      Self::Colima { profile } => {
        if profile == "default" {
          "Colima".to_string()
        } else {
          format!("Colima ({profile})")
        }
      }
      Self::NativeDocker { .. } => "Docker".to_string(),
      Self::Wsl2Docker { distro, .. } => format!("Docker (WSL2: {distro})"),
      Self::RootlessDocker { .. } => "Docker (Rootless)".to_string(),
      Self::Custom { .. } => "Docker (Custom)".to_string(),
    }
  }

  /// Create a Colima runtime with the default profile
  #[must_use]
  pub fn colima_default() -> Self {
    Self::Colima {
      profile: "default".to_string(),
    }
  }

  /// Create a native Docker runtime with the default socket path
  #[must_use]
  pub fn native_default() -> Self {
    Self::NativeDocker {
      socket_path: "/var/run/docker.sock".to_string(),
    }
  }

  /// Create a rootless Docker runtime for the current user
  #[must_use]
  pub fn rootless_default() -> Self {
    let uid = Self::get_current_uid();
    Self::RootlessDocker {
      socket_path: format!("/run/user/{uid}/docker.sock"),
    }
  }

  /// Create a WSL2 Docker runtime with default port
  #[must_use]
  pub fn wsl2_default(distro: String) -> Self {
    Self::Wsl2Docker {
      distro,
      port: Self::DEFAULT_DOCKER_TCP_PORT,
    }
  }

  /// Get current user ID
  /// On Unix, reads from /proc/self/status or uses a reasonable default
  fn get_current_uid() -> u32 {
    #[cfg(unix)]
    {
      // Try reading from /proc/self/status
      if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
        for line in content.lines() {
          if line.starts_with("Uid:") {
            // Format: "Uid:\t1000\t1000\t1000\t1000"
            if let Some(uid_str) = line.split_whitespace().nth(1)
              && let Ok(uid) = uid_str.parse::<u32>()
            {
              return uid;
            }
          }
        }
      }

      // Fallback: try to get from environment or use default
      if let Ok(uid_str) = std::env::var("UID")
        && let Ok(uid) = uid_str.parse::<u32>()
      {
        return uid;
      }

      // Default non-root user ID
      1000
    }

    #[cfg(not(unix))]
    {
      1000 // Default UID for Windows compatibility
    }
  }

  /// Detect all available Docker runtimes on the current system
  #[must_use]
  pub fn detect_available() -> Vec<Self> {
    let platform = Platform::detect();
    let mut runtimes = Vec::new();

    match platform {
      Platform::MacOS => {
        // Check for Colima profiles
        Self::detect_colima_profiles(&mut runtimes);

        // Check for Docker Desktop socket
        let docker_desktop = "/var/run/docker.sock";
        if Path::new(docker_desktop).exists() {
          runtimes.push(Self::NativeDocker {
            socket_path: docker_desktop.to_string(),
          });
        }
      }
      Platform::Linux | Platform::WindowsWsl2 => {
        // Check native Docker socket first
        let native_socket = "/var/run/docker.sock";
        if Path::new(native_socket).exists() {
          runtimes.push(Self::NativeDocker {
            socket_path: native_socket.to_string(),
          });
        }

        // Check rootless Docker
        let rootless = Self::rootless_default();
        if rootless.is_available() {
          runtimes.push(rootless);
        }

        // Check for Colima profiles
        Self::detect_colima_profiles(&mut runtimes);
      }
      Platform::Windows => {
        // On native Windows, detect WSL2 distros with Docker
        #[cfg(target_os = "windows")]
        {
          if let Ok(distros) = super::wsl::list_distros() {
            for distro in distros {
              if distro.version == 2 {
                if let Some(port) = distro.docker_port {
                  runtimes.push(Self::Wsl2Docker {
                    distro: distro.name,
                    port,
                  });
                }
              }
            }
          }
        }

        // If no WSL2 runtimes found, add a default one that user can configure
        if runtimes.is_empty() {
          runtimes.push(Self::Wsl2Docker {
            distro: "Ubuntu".to_string(),
            port: Self::DEFAULT_DOCKER_TCP_PORT,
          });
        }
      }
    }

    runtimes
  }

  /// Detect Colima profiles and add them to the runtimes list
  fn detect_colima_profiles(runtimes: &mut Vec<Self>) {
    let Some(home) = dirs::home_dir() else {
      return;
    };

    let colima_dir = home.join(".colima");
    if !colima_dir.exists() {
      return;
    }

    // List directories in ~/.colima that contain docker.sock
    if let Ok(entries) = std::fs::read_dir(&colima_dir) {
      for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
          let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
          // Skip internal directories
          if name.starts_with('_') {
            continue;
          }
          let socket = path.join("docker.sock");
          if socket.exists() {
            runtimes.push(Self::Colima {
              profile: name.to_string(),
            });
          }
        }
      }
    }
  }

  /// Get the default runtime for the current platform
  #[must_use]
  pub fn default_for_platform() -> Self {
    let platform = Platform::detect();
    match platform {
      Platform::MacOS => Self::colima_default(),
      Platform::Linux | Platform::WindowsWsl2 => Self::native_default(),
      Platform::Windows => Self::Wsl2Docker {
        distro: "Ubuntu".to_string(),
        port: Self::DEFAULT_DOCKER_TCP_PORT,
      },
    }
  }
}

impl Default for DockerRuntime {
  fn default() -> Self {
    Self::default_for_platform()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_colima_connection_string() {
    let runtime = DockerRuntime::Colima {
      profile: "default".to_string(),
    };
    let conn = runtime.connection_string();
    assert!(conn.ends_with("/.colima/default/docker.sock"));
  }

  #[test]
  fn test_native_docker_connection_string() {
    let runtime = DockerRuntime::NativeDocker {
      socket_path: "/var/run/docker.sock".to_string(),
    };
    assert_eq!(runtime.connection_string(), "/var/run/docker.sock");
  }

  #[test]
  fn test_wsl2_connection_string() {
    let runtime = DockerRuntime::Wsl2Docker {
      distro: "Ubuntu".to_string(),
      port: 2375,
    };
    assert_eq!(runtime.connection_string(), "http://localhost:2375");
  }

  #[test]
  fn test_uses_tcp() {
    assert!(!DockerRuntime::colima_default().uses_tcp());
    assert!(!DockerRuntime::native_default().uses_tcp());
    assert!(DockerRuntime::wsl2_default("Ubuntu".to_string()).uses_tcp());
    assert!(
      DockerRuntime::Custom {
        connection_string: "http://localhost:2375".to_string()
      }
      .uses_tcp()
    );
  }

  #[test]
  fn test_display_name() {
    assert_eq!(DockerRuntime::colima_default().display_name(), "Colima");
    assert_eq!(
      DockerRuntime::Colima {
        profile: "dev".to_string()
      }
      .display_name(),
      "Colima (dev)"
    );
    assert_eq!(DockerRuntime::native_default().display_name(), "Docker");
    assert_eq!(
      DockerRuntime::wsl2_default("Ubuntu".to_string()).display_name(),
      "Docker (WSL2: Ubuntu)"
    );
  }

  #[test]
  fn test_detect_available_returns_list() {
    // Should not panic and returns a Vec (length is unsigned, so existence is enough).
    let _ = DockerRuntime::detect_available();
  }

  #[test]
  fn test_default_for_platform() {
    let runtime = DockerRuntime::default_for_platform();
    // Should return a valid runtime for the current platform
    assert!(!runtime.display_name().is_empty());
  }
}
