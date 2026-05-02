//! Kubernetes setup diagnostics.
//!
//! Distinguishes between "kubectl/k8s not installed" and "k8s installed but
//! cluster unreachable" so the UI can show targeted setup instructions
//! instead of a generic load failure.

use std::path::PathBuf;

use crate::platform::Platform;
use crate::utils::find_binary;

/// Detected state of the local Kubernetes setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum K8sStatus {
  /// `kubectl` not on PATH and we couldn't find it in any platform-specific dir.
  KubectlMissing,
  /// `kubectl` exists but no `~/.kube/config` and no `KUBECONFIG` env var.
  KubeconfigMissing,
  /// `kubectl` + kubeconfig present but the original error was something else
  /// (most commonly: cluster not running / unreachable). The original error
  /// message is preserved so the UI can show it.
  ClusterUnreachable(String),
}

impl K8sStatus {
  /// Diagnose the current state. `original_error` is the error message that
  /// triggered this lookup (e.g., "Failed to load kubeconfig" from the kube
  /// crate). Used as the fallback `ClusterUnreachable` payload.
  #[must_use]
  pub fn diagnose(original_error: &str) -> Self {
    if find_binary("kubectl").is_none() {
      return Self::KubectlMissing;
    }
    if !kubeconfig_exists() {
      return Self::KubeconfigMissing;
    }
    Self::ClusterUnreachable(original_error.to_string())
  }
}

/// Path to the user's kubeconfig honoring `$KUBECONFIG`, falling back to `~/.kube/config`.
fn kubeconfig_path() -> Option<PathBuf> {
  if let Ok(p) = std::env::var("KUBECONFIG")
    && !p.is_empty()
  {
    // KUBECONFIG can be a colon-separated list; take the first non-empty entry.
    let first = p.split(if cfg!(windows) { ';' } else { ':' }).find(|s| !s.is_empty());
    if let Some(first) = first {
      return Some(PathBuf::from(first));
    }
  }
  dirs::home_dir().map(|home| home.join(".kube").join("config"))
}

fn kubeconfig_exists() -> bool {
  kubeconfig_path().is_some_and(|p| p.exists())
}

/// Platform-specific install command for `kubectl`. Returned as
/// `(command, description)` for display.
#[must_use]
pub fn kubectl_install_hint() -> (&'static str, &'static str) {
  match Platform::detect() {
    Platform::MacOS => (
      "brew install kubectl",
      "Install kubectl with Homebrew. Visit https://brew.sh first if you don't have it.",
    ),
    Platform::Linux | Platform::WindowsWsl2 => (
      "curl -LO https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl && chmod +x kubectl && sudo mv kubectl /usr/local/bin/",
      "Download kubectl from the official Kubernetes release. Your distro's package manager (apt/dnf/pacman) is also a good option.",
    ),
    Platform::Windows => (
      "winget install -e --id Kubernetes.kubectl",
      "Install kubectl on Windows via winget, or use Chocolatey: choco install kubernetes-cli.",
    ),
  }
}

/// Hints for getting a working kubeconfig once `kubectl` is installed.
#[must_use]
pub fn kubeconfig_setup_hint() -> (&'static str, &'static str) {
  match Platform::detect() {
    Platform::MacOS | Platform::Linux => (
      "colima start --kubernetes",
      "Start Colima with Kubernetes enabled — it writes ~/.kube/config automatically. Or point KUBECONFIG at an existing cluster's kubeconfig file.",
    ),
    Platform::WindowsWsl2 => (
      "colima start --kubernetes",
      "Start Colima with Kubernetes enabled inside your WSL2 distro. Or point KUBECONFIG at an existing kubeconfig.",
    ),
    Platform::Windows => (
      "kubectl config view",
      "Configure your kubeconfig: set the KUBECONFIG environment variable, or place a config at %USERPROFILE%\\.kube\\config.",
    ),
  }
}
