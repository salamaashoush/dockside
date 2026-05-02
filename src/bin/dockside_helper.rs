//! Privileged helper binary for `dockside`.
//!
//! Exposes a narrow CLI surface that the main app shells out to via the
//! platform's standard authentication wrapper:
//!   * macOS: `osascript -e 'do shell script "..." with administrator privileges'`
//!   * Linux: `pkexec /path/to/dockside-helper ...`
//!
//! Subcommands are intentionally tiny so each one is auditable and so the
//! UAC/sudo prompt only happens for narrowly-scoped actions.

#![allow(clippy::print_stdout, clippy::print_stderr)] // CLI tool; output is the product.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().collect();
  let cmd = args.get(1).map_or("", String::as_str);
  let result = match cmd {
    "install-resolver" => install_resolver(args.get(2..).unwrap_or(&[])),
    "uninstall-resolver" => uninstall_resolver(args.get(2..).unwrap_or(&[])),
    "install-ca" => install_ca(args.get(2..).unwrap_or(&[])),
    "uninstall-ca" => uninstall_ca(args.get(2..).unwrap_or(&[])),
    "version" => {
      println!("dockside-helper {}", env!("CARGO_PKG_VERSION"));
      Ok(())
    }
    other => {
      eprintln!("usage: dockside-helper <install-resolver|uninstall-resolver|install-ca|uninstall-ca|version> ...");
      eprintln!("got: {other:?}");
      return ExitCode::from(64);
    }
  };
  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("dockside-helper: {e:#}");
      ExitCode::from(1)
    }
  }
}

/// `install-resolver <suffix> <port>` — register a per-domain DNS resolver
/// pointing at `127.0.0.1:<port>` for `<suffix>` only.
fn install_resolver(args: &[String]) -> anyhow::Result<()> {
  let suffix = args.first().ok_or_else(|| anyhow::anyhow!("missing <suffix> arg"))?;
  let port: u16 = args
    .get(1)
    .ok_or_else(|| anyhow::anyhow!("missing <port> arg"))?
    .parse()
    .map_err(|e| anyhow::anyhow!("invalid port: {e}"))?;
  validate_suffix(suffix)?;

  #[cfg(target_os = "macos")]
  {
    install_resolver_macos(suffix, port)
  }
  #[cfg(target_os = "linux")]
  {
    install_resolver_linux(suffix, port)
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = (suffix, port);
    anyhow::bail!("unsupported OS — only macOS and Linux are wired today");
  }
}

fn uninstall_resolver(args: &[String]) -> anyhow::Result<()> {
  let suffix = args.first().ok_or_else(|| anyhow::anyhow!("missing <suffix> arg"))?;
  validate_suffix(suffix)?;

  #[cfg(target_os = "macos")]
  {
    uninstall_resolver_macos(suffix)
  }
  #[cfg(target_os = "linux")]
  {
    uninstall_resolver_linux(suffix)
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = suffix;
    anyhow::bail!("unsupported OS");
  }
}

/// `install-ca <pem-path>` — copy the local CA cert into the OS trust
/// store. Runs with privilege.
fn install_ca(args: &[String]) -> anyhow::Result<()> {
  let pem_path: PathBuf = args
    .first()
    .ok_or_else(|| anyhow::anyhow!("missing <pem-path> arg"))?
    .into();
  if !pem_path.is_file() {
    anyhow::bail!("CA file does not exist: {}", pem_path.display());
  }

  #[cfg(target_os = "macos")]
  {
    install_ca_macos(&pem_path)
  }
  #[cfg(target_os = "linux")]
  {
    install_ca_linux(&pem_path)
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = pem_path;
    anyhow::bail!("unsupported OS");
  }
}

fn uninstall_ca(_args: &[String]) -> anyhow::Result<()> {
  #[cfg(target_os = "macos")]
  {
    uninstall_ca_macos()
  }
  #[cfg(target_os = "linux")]
  {
    uninstall_ca_linux()
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    anyhow::bail!("unsupported OS");
  }
}

fn validate_suffix(suffix: &str) -> anyhow::Result<()> {
  let s = suffix.trim_matches('.');
  if s.is_empty() || s.contains(char::is_whitespace) || s.contains('/') {
    anyhow::bail!("invalid suffix: {suffix:?}");
  }
  // Reject mDNS reservation explicitly. clippy flags `.ends_with(".local")` as
  // a "case-sensitive file extension comparison"; we already lowercased so it
  // is correct here. We compare label-by-label to make that intent explicit.
  let last_label = s.rsplit('.').next().unwrap_or(s).to_ascii_lowercase();
  if last_label == "local" {
    anyhow::bail!("`.local` is reserved for mDNS — pick a different suffix");
  }
  Ok(())
}

// =====================================================================
// macOS
// =====================================================================

#[cfg(target_os = "macos")]
fn install_resolver_macos(suffix: &str, port: u16) -> anyhow::Result<()> {
  use std::fs::OpenOptions;
  use std::os::unix::fs::PermissionsExt;
  let path = std::path::PathBuf::from(format!("/etc/resolver/{suffix}"));
  std::fs::create_dir_all("/etc/resolver")?;
  let body = format!("nameserver 127.0.0.1\nport {port}\nsearch_order 1\n");
  let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(&path)?;
  f.write_all(body.as_bytes())?;
  f.set_permissions(std::fs::Permissions::from_mode(0o644))?;
  println!("installed {}", path.display());
  Ok(())
}

#[cfg(target_os = "macos")]
fn uninstall_resolver_macos(suffix: &str) -> anyhow::Result<()> {
  let path = std::path::PathBuf::from(format!("/etc/resolver/{suffix}"));
  if path.exists() {
    std::fs::remove_file(&path)?;
    println!("removed {}", path.display());
  }
  Ok(())
}

#[cfg(target_os = "macos")]
fn install_ca_macos(pem_path: &std::path::Path) -> anyhow::Result<()> {
  let status = std::process::Command::new("security")
    .args([
      "add-trusted-cert",
      "-d",
      "-r",
      "trustRoot",
      "-k",
      "/Library/Keychains/System.keychain",
    ])
    .arg(pem_path)
    .status()?;
  if !status.success() {
    anyhow::bail!("`security add-trusted-cert` exited with {status}");
  }
  println!("installed CA into System.keychain");
  Ok(())
}

#[cfg(target_os = "macos")]
fn uninstall_ca_macos() -> anyhow::Result<()> {
  let status = std::process::Command::new("security")
    .args([
      "delete-certificate",
      "-c",
      "Dockside Local Root CA",
      "/Library/Keychains/System.keychain",
    ])
    .status()?;
  if !status.success() {
    eprintln!("warning: delete-certificate exited with {status}");
  }
  Ok(())
}

// =====================================================================
// Linux
// =====================================================================

#[cfg(target_os = "linux")]
fn install_resolver_linux(suffix: &str, port: u16) -> anyhow::Result<()> {
  use std::path::{Path, PathBuf};
  // Prefer NetworkManager dnsmasq plugin (per-domain forwarding with
  // explicit port) when present; fall back to systemd-resolved drop-in.
  if Path::new("/etc/NetworkManager/dnsmasq.d").is_dir() {
    let path = PathBuf::from(format!("/etc/NetworkManager/dnsmasq.d/dockside-{suffix}.conf"));
    let body = format!("server=/{suffix}/127.0.0.1#{port}\n");
    write_root_file(&path, body.as_bytes(), 0o644)?;
    let _ = std::process::Command::new("nmcli").args(["general", "reload"]).status();
    println!("installed {}", path.display());
    return Ok(());
  }

  // systemd-resolved fallback. Requires a custom .network drop-in tied to
  // the loopback interface so the routing-only domain `~suffix` points at
  // the local resolver. systemd-resolved supports `port` syntax via
  // `DNS=127.0.0.1:port` in v247+ — caller must have a recent distro.
  let dir = Path::new("/etc/systemd/resolved.conf.d");
  std::fs::create_dir_all(dir)?;
  let path = dir.join(format!("dockside-{suffix}.conf"));
  let body = format!("[Resolve]\nDNS=127.0.0.1:{port}\nDomains=~{suffix}\n");
  write_root_file(&path, body.as_bytes(), 0o644)?;
  let _ = std::process::Command::new("systemctl")
    .args(["restart", "systemd-resolved"])
    .status();
  println!("installed {}", path.display());
  Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_resolver_linux(suffix: &str) -> anyhow::Result<()> {
  use std::path::PathBuf;
  for candidate in [
    PathBuf::from(format!("/etc/NetworkManager/dnsmasq.d/dockside-{suffix}.conf")),
    PathBuf::from(format!("/etc/systemd/resolved.conf.d/dockside-{suffix}.conf")),
  ] {
    if candidate.exists() {
      std::fs::remove_file(&candidate)?;
      println!("removed {}", candidate.display());
    }
  }
  let _ = std::process::Command::new("systemctl")
    .args(["restart", "systemd-resolved"])
    .status();
  let _ = std::process::Command::new("nmcli").args(["general", "reload"]).status();
  Ok(())
}

#[cfg(target_os = "linux")]
fn install_ca_linux(pem_path: &std::path::Path) -> anyhow::Result<()> {
  let dest_dir = std::path::Path::new("/usr/local/share/ca-certificates");
  std::fs::create_dir_all(dest_dir)?;
  let dest = dest_dir.join("dockside.crt");
  std::fs::copy(pem_path, &dest)?;
  let status = std::process::Command::new("update-ca-certificates").status()?;
  if !status.success() {
    anyhow::bail!("`update-ca-certificates` exited with {status}");
  }
  println!("installed CA at {}", dest.display());
  Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_ca_linux() -> anyhow::Result<()> {
  let path = std::path::Path::new("/usr/local/share/ca-certificates/dockside.crt");
  if path.exists() {
    std::fs::remove_file(path)?;
    let _ = std::process::Command::new("update-ca-certificates").status();
    println!("removed {}", path.display());
  }
  Ok(())
}

#[cfg(unix)]
fn write_root_file(path: &std::path::Path, contents: &[u8], mode: u32) -> anyhow::Result<()> {
  use std::fs::OpenOptions;
  use std::os::unix::fs::OpenOptionsExt;
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  let mut f = OpenOptions::new()
    .create(true)
    .write(true)
    .truncate(true)
    .mode(mode)
    .open(path)?;
  f.write_all(contents)?;
  Ok(())
}
