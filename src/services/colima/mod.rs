//! Colima machine and Kubernetes control operations
//!
//! This module is only available on macOS and Linux platforms.

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod kubernetes;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod machines;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use kubernetes::*;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use machines::*;

// Stub implementations for Windows
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod stubs {
  use gpui::App;

  /// Start Colima stub
  pub fn start_colima(_profile: Option<&str>, _cx: &mut App) {
    tracing::warn!("Colima is not supported on this platform");
  }

  /// Stop Colima stub
  pub fn stop_colima(_profile: Option<&str>, _cx: &mut App) {
    tracing::warn!("Colima is not supported on this platform");
  }

  /// Restart Colima stub
  pub fn restart_colima(_profile: Option<&str>, _cx: &mut App) {
    tracing::warn!("Colima is not supported on this platform");
  }

  /// Enable Kubernetes stub
  pub fn enable_kubernetes(_profile: Option<&str>, _cx: &mut App) {
    tracing::warn!("Colima is not supported on this platform");
  }

  /// Disable Kubernetes stub
  pub fn disable_kubernetes(_profile: Option<&str>, _cx: &mut App) {
    tracing::warn!("Colima is not supported on this platform");
  }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub use stubs::*;
