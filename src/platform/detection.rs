//! Platform detection logic

use std::path::Path;

/// Supported platforms for Dockside
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Windows variant is only constructed on Windows builds
pub enum Platform {
  /// macOS (Apple Silicon or Intel)
  MacOS,
  /// Native Linux
  Linux,
  /// Windows (native)
  Windows,
  /// Running inside WSL2 on Windows
  WindowsWsl2,
}

impl Platform {
  /// Detect the current platform at runtime
  #[must_use]
  pub fn detect() -> Self {
    #[cfg(target_os = "macos")]
    {
      Self::MacOS
    }

    #[cfg(target_os = "linux")]
    {
      // Check if we're running inside WSL2
      if Self::is_wsl2() {
        Self::WindowsWsl2
      } else {
        Self::Linux
      }
    }

    #[cfg(target_os = "windows")]
    {
      Self::Windows
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
      // Default to Linux for other Unix-like systems
      Self::Linux
    }
  }

  /// Check if running inside WSL2
  #[cfg(target_os = "linux")]
  fn is_wsl2() -> bool {
    // Check for WSL-specific indicators
    if Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists() {
      return true;
    }

    // Check /proc/version for Microsoft/WSL string
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
      let version_lower = version.to_lowercase();
      if version_lower.contains("microsoft") || version_lower.contains("wsl") {
        return true;
      }
    }

    // Check WSL_DISTRO_NAME environment variable
    std::env::var("WSL_DISTRO_NAME").is_ok()
  }

  /// Returns whether this platform supports Colima
  /// Colima is available on macOS and Linux (including `WSL2`)
  #[must_use]
  pub const fn supports_colima(self) -> bool {
    matches!(self, Self::MacOS | Self::Linux | Self::WindowsWsl2)
  }

  /// Returns whether this platform uses Unix sockets for Docker
  #[must_use]
  pub const fn uses_unix_socket(self) -> bool {
    matches!(self, Self::MacOS | Self::Linux | Self::WindowsWsl2)
  }

  /// Returns whether this platform requires TCP connection for Docker
  /// (Windows native connecting to Docker in `WSL2`)
  #[must_use]
  pub const fn requires_tcp_connection(self) -> bool {
    matches!(self, Self::Windows)
  }

  /// Returns the display name for this platform
  #[must_use]
  pub const fn display_name(self) -> &'static str {
    match self {
      Self::MacOS => "macOS",
      Self::Linux => "Linux",
      Self::Windows => "Windows",
      Self::WindowsWsl2 => "WSL2",
    }
  }
}

impl Default for Platform {
  fn default() -> Self {
    Self::detect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_platform_detect_returns_valid_platform() {
    let platform = Platform::detect();
    // Should return one of the valid platforms
    assert!(matches!(
      platform,
      Platform::MacOS | Platform::Linux | Platform::Windows | Platform::WindowsWsl2
    ));
  }

  #[test]
  fn test_platform_display_name() {
    assert_eq!(Platform::MacOS.display_name(), "macOS");
    assert_eq!(Platform::Linux.display_name(), "Linux");
    assert_eq!(Platform::Windows.display_name(), "Windows");
    assert_eq!(Platform::WindowsWsl2.display_name(), "WSL2");
  }

  #[test]
  fn test_supports_colima() {
    assert!(Platform::MacOS.supports_colima());
    assert!(Platform::Linux.supports_colima());
    assert!(Platform::WindowsWsl2.supports_colima());
    assert!(!Platform::Windows.supports_colima());
  }

  #[test]
  fn test_uses_unix_socket() {
    assert!(Platform::MacOS.uses_unix_socket());
    assert!(Platform::Linux.uses_unix_socket());
    assert!(Platform::WindowsWsl2.uses_unix_socket());
    assert!(!Platform::Windows.uses_unix_socket());
  }

  #[test]
  fn test_requires_tcp_connection() {
    assert!(!Platform::MacOS.requires_tcp_connection());
    assert!(!Platform::Linux.requires_tcp_connection());
    assert!(!Platform::WindowsWsl2.requires_tcp_connection());
    assert!(Platform::Windows.requires_tcp_connection());
  }

  #[test]
  fn test_platform_default() {
    let default = Platform::default();
    let detected = Platform::detect();
    assert_eq!(default, detected);
  }
}
