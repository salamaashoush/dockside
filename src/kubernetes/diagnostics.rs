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

/// First kubeconfig path in `$KUBECONFIG`, if set and non-empty.
fn kubeconfig_env_path() -> Option<PathBuf> {
  let p = std::env::var("KUBECONFIG").ok()?;
  if p.is_empty() {
    return None;
  }
  let sep = if cfg!(windows) { ';' } else { ':' };
  p.split(sep).find(|s| !s.is_empty()).map(PathBuf::from)
}

/// First detected kubeconfig path that actually exists on disk, if any.
#[must_use]
pub fn first_existing_known_kubeconfig() -> Option<PathBuf> {
  known_kubeconfig_paths().into_iter().find(|p| p.exists())
}

/// Well-known kubeconfig locations across local k8s flavors. Order
/// matters: anything user-pointed (`KUBECONFIG`, `~/.kube/config`) wins
/// over distro-managed configs.
fn known_kubeconfig_paths() -> Vec<PathBuf> {
  let mut out = Vec::new();
  if let Some(p) = kubeconfig_env_path() {
    out.push(p);
  }
  if let Some(home) = dirs::home_dir() {
    out.push(home.join(".kube").join("config"));
    // microk8s on Linux/macOS via snap.
    out.push(
      home
        .join("snap")
        .join("microk8s")
        .join("current")
        .join("credentials")
        .join("client.config"),
    );
  }
  // k3s default install (no setup script run).
  out.push(PathBuf::from("/etc/rancher/k3s/k3s.yaml"));
  // kubeadm-installed clusters.
  out.push(PathBuf::from("/etc/kubernetes/admin.conf"));
  out
}

fn kubeconfig_exists() -> bool {
  known_kubeconfig_paths().iter().any(|p| p.exists())
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
/// Generic across distros — names the common local-cluster options
/// (k3s, kubeadm, colima, microk8s, Docker Desktop) without prescribing
/// one. If we detected a known kubeconfig path that the kube crate
/// can't read directly (e.g. `/etc/rancher/k3s/k3s.yaml` is root-owned),
/// the second tuple element points at it so the user can copy or
/// `export KUBECONFIG`.
#[must_use]
pub fn kubeconfig_setup_hint() -> (&'static str, &'static str) {
  // Surface the first known-but-not-default path we find — most likely
  // user has a cluster, just hasn't pointed kubectl at it.
  for p in known_kubeconfig_paths() {
    if p.exists() {
      // Found something at a non-default location; suggest pointing
      // KUBECONFIG at it.
      let s = p.to_string_lossy();
      // Leak the formatted string so we can return &'static. Acceptable:
      // hint runs once on error path.
      let cmd: &'static str = Box::leak(format!("export KUBECONFIG={s}").into_boxed_str());
      return (
        cmd,
        "A kubeconfig was found at a non-default location. Export this in your shell or set the kubeconfig path in Dockside Settings.",
      );
    }
  }
  match Platform::detect() {
    Platform::MacOS | Platform::Linux | Platform::WindowsWsl2 => (
      "kubectl config view",
      "No kubeconfig found. Start a cluster (k3s, kubeadm, colima --kubernetes, microk8s, kind, minikube, Docker Desktop, …) so kubectl writes ~/.kube/config, or set KUBECONFIG at an existing one.",
    ),
    Platform::Windows => (
      "kubectl config view",
      "Configure your kubeconfig: set the KUBECONFIG environment variable, or place a config at %USERPROFILE%\\.kube\\config (Docker Desktop, kind, minikube, etc. write it for you).",
    ),
  }
}
