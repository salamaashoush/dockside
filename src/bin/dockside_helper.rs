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
    "bootstrap" => bootstrap(args.get(2..).unwrap_or(&[])),
    "uninstall-bootstrap" => uninstall_bootstrap(),
    "grant-low-ports" => grant_low_ports(args.get(2..).unwrap_or(&[])),
    "revoke-low-ports" => revoke_low_ports(args.get(2..).unwrap_or(&[])),
    "version" => {
      println!("dockside-helper {}", env!("CARGO_PKG_VERSION"));
      Ok(())
    }
    other => {
      eprintln!(
        "usage: dockside-helper <subcommand> ...\n\
         subcommands: install-resolver, uninstall-resolver, install-ca, uninstall-ca,\n\
                      bootstrap, uninstall-bootstrap, grant-low-ports, revoke-low-ports, version"
      );
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
/// One-time setup: copy the helper to a stable system location, write a
/// `PolicyKit` policy that lets the user invoke it via pkexec without a
/// password each time, and (optionally) install the local CA + register
/// the resolver in one go. Runs as root via pkexec; subsequent ops use the
/// cached admin auth so the user only sees the prompt once per session.
///
/// Args: `bootstrap <suffix> <port> [<ca-pem-path>]`
fn bootstrap(args: &[String]) -> anyhow::Result<()> {
  let suffix = args.first().ok_or_else(|| anyhow::anyhow!("missing <suffix> arg"))?;
  let port: u16 = args
    .get(1)
    .ok_or_else(|| anyhow::anyhow!("missing <port> arg"))?
    .parse()
    .map_err(|e| anyhow::anyhow!("invalid port: {e}"))?;
  validate_suffix(suffix)?;
  let ca_pem = args.get(2).map(PathBuf::from);

  #[cfg(target_os = "linux")]
  {
    bootstrap_linux(suffix, port, ca_pem.as_deref())
  }
  #[cfg(target_os = "macos")]
  {
    bootstrap_macos(suffix, port, ca_pem.as_deref())
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos")))]
  {
    let _ = (suffix, port, ca_pem);
    anyhow::bail!("bootstrap not implemented for this OS");
  }
}

// Linux branch does real work; macOS branch is a no-op. The unified
// signature exists because `main()` dispatches generically.
#[allow(clippy::unnecessary_wraps)]
fn uninstall_bootstrap() -> anyhow::Result<()> {
  #[cfg(target_os = "linux")]
  {
    uninstall_bootstrap_linux()
  }
  #[cfg(target_os = "macos")]
  {
    Ok(())
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos")))]
  {
    Ok(())
  }
}

#[cfg(target_os = "linux")]
fn bootstrap_linux(suffix: &str, port: u16, ca_pem: Option<&std::path::Path>) -> anyhow::Result<()> {
  use std::os::unix::fs::PermissionsExt;
  use std::path::Path;

  // 1. Copy the running helper to a stable system path so the polkit
  //    policy can reference it without depending on the user's source tree.
  let self_exe = std::env::current_exe()?;
  let target = Path::new("/usr/local/libexec/dockside-helper");
  std::fs::create_dir_all("/usr/local/libexec")?;
  std::fs::copy(&self_exe, target)?;
  let mut perms = std::fs::metadata(target)?.permissions();
  perms.set_mode(0o755);
  std::fs::set_permissions(target, perms)?;
  println!("installed helper at {}", target.display());

  // 2. Drop a PolicyKit rule allowing pkexec to run the helper after a
  //    single admin authentication that lasts the rest of the user's
  //    session (`auth_admin_keep`).
  let policy_dir = Path::new("/usr/share/polkit-1/actions");
  std::fs::create_dir_all(policy_dir)?;
  let policy_path = policy_dir.join("dev.dockside.helper.policy");
  let policy = format!(
    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC
 "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <action id="dev.dockside.helper">
    <description>Manage the local *.{suffix} resolver and HTTPS root CA</description>
    <message>Dockside needs to register the local DNS resolver and trust its root CA.</message>
    <defaults>
      <allow_any>auth_admin_keep</allow_any>
      <allow_inactive>auth_admin_keep</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">{}</annotate>
  </action>
</policyconfig>
"#,
    target.display()
  );
  write_root_file(&policy_path, policy.as_bytes(), 0o644)?;
  println!("installed polkit policy at {}", policy_path.display());

  // 3. Install resolver + CA in the same privileged pass.
  install_resolver_linux(suffix, port)?;
  if let Some(pem) = ca_pem
    && pem.is_file()
  {
    install_ca_linux(pem)?;
  }
  Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_bootstrap_linux() -> anyhow::Result<()> {
  use std::path::Path;
  for path in [
    Path::new("/usr/share/polkit-1/actions/dev.dockside.helper.policy"),
    Path::new("/usr/local/libexec/dockside-helper"),
  ] {
    if path.exists() {
      std::fs::remove_file(path)?;
      println!("removed {}", path.display());
    }
  }
  Ok(())
}

#[cfg(target_os = "macos")]
fn bootstrap_macos(suffix: &str, port: u16, ca_pem: Option<&std::path::Path>) -> anyhow::Result<()> {
  // macOS does not need a polkit-equivalent rule; `osascript ... with
  // administrator privileges` already caches sudo for ~5 min by default.
  // Just install resolver + CA in one privileged pass.
  install_resolver_macos(suffix, port)?;
  if let Some(pem) = ca_pem
    && pem.is_file()
  {
    install_ca_macos(pem)?;
  }
  Ok(())
}

/// `grant-low-ports <binary-path>` — grant `cap_net_bind_service` on the
/// given executable so it can bind ports below 1024 without root. Lets
/// users drop the `:47080`/`:47443` from their dev URLs.
fn grant_low_ports(args: &[String]) -> anyhow::Result<()> {
  let binary = args
    .first()
    .ok_or_else(|| anyhow::anyhow!("missing <binary-path> arg"))?;
  if !std::path::Path::new(binary).is_file() {
    anyhow::bail!("binary does not exist: {binary}");
  }

  #[cfg(target_os = "linux")]
  {
    let _ = binary;
    install_redirect_linux()
  }
  #[cfg(target_os = "macos")]
  {
    // macOS does not expose Linux-style file capabilities. The standard
    // alternative is a pf redirect; install one that maps 80/443 → 47080/47443
    // for loopback traffic only.
    let pf_anchor = "/etc/pf.anchors/dockside";
    let body = "rdr pass on lo0 inet proto tcp from any to 127.0.0.1 port 80 -> 127.0.0.1 port 47080\n\
                rdr pass on lo0 inet proto tcp from any to 127.0.0.1 port 443 -> 127.0.0.1 port 47443\n";
    write_root_file(std::path::Path::new(pf_anchor), body.as_bytes(), 0o644)?;
    let status = std::process::Command::new("/sbin/pfctl")
      .args(["-a", "dockside", "-f", pf_anchor])
      .status()?;
    if !status.success() {
      anyhow::bail!("pfctl load exited with {status}");
    }
    let _ = binary; // unused on macOS path
    println!("loaded pf anchor {pf_anchor} mapping 80/443 → 47080/47443");
    Ok(())
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos")))]
  {
    let _ = binary;
    anyhow::bail!("grant-low-ports not implemented for this OS");
  }
}

// Linux branch does real work; macOS branch is a no-op. The unified
// signature exists because `main()` dispatches generically.
#[allow(clippy::unnecessary_wraps)]
fn revoke_low_ports(args: &[String]) -> anyhow::Result<()> {
  #[cfg(target_os = "linux")]
  {
    let _ = args;
    uninstall_redirect_linux()
  }
  #[cfg(target_os = "macos")]
  {
    let _ = args;
    let _ = std::process::Command::new("/sbin/pfctl")
      .args(["-a", "dockside", "-F", "all"])
      .status();
    let _ = std::fs::remove_file("/etc/pf.anchors/dockside");
    Ok(())
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos")))]
  {
    let _ = args;
    Ok(())
  }
}

#[cfg(target_os = "linux")]
const REDIRECT_NFT_PATH: &str = "/usr/local/lib/dockside/redirect.nft";
#[cfg(target_os = "linux")]
const REDIRECT_UNIT_PATH: &str = "/etc/systemd/system/dockside-port-redirect.service";
#[cfg(target_os = "linux")]
const REDIRECT_UNIT_NAME: &str = "dockside-port-redirect.service";

/// Install an `nftables` `nat` table that redirects loopback connections
/// to 80/443 to the unprivileged ports the dockside proxy actually binds
/// (47080/47443). Persisted via a `oneshot` systemd unit so it survives
/// reboot. Survives `cargo build` rebuilds — unlike file capabilities,
/// the redirect lives in the kernel's netfilter state, independent of
/// the binary on disk.
#[cfg(target_os = "linux")]
fn install_redirect_linux() -> anyhow::Result<()> {
  let nft = find_tool(&["/usr/sbin/nft", "/usr/bin/nft", "/sbin/nft"])
    .ok_or_else(|| anyhow::anyhow!("`nft` not found — install the `nftables` package"))?;

  // Atomic ruleset: declare the table first to make `delete` safe even on
  // a fresh system, then redeclare with the actual rules. The whole file
  // is loaded as a single transaction by `nft -f` so no half-state.
  // Restricted to `daddr 127.0.0.1` so external traffic is untouched.
  let ruleset = "table ip dockside_nat {\n\
                 \tchain output { type nat hook output priority -100; }\n\
                 }\n\
                 delete table ip dockside_nat\n\
                 table ip dockside_nat {\n\
                 \tchain output {\n\
                 \t\ttype nat hook output priority -100;\n\
                 \t\tip daddr 127.0.0.1 tcp dport 80 redirect to :47080\n\
                 \t\tip daddr 127.0.0.1 tcp dport 443 redirect to :47443\n\
                 \t}\n\
                 }\n";
  write_root_file(std::path::Path::new(REDIRECT_NFT_PATH), ruleset.as_bytes(), 0o644)?;
  println!("wrote {REDIRECT_NFT_PATH}");

  // Apply now.
  let status = std::process::Command::new(&nft)
    .args(["-f", REDIRECT_NFT_PATH])
    .status()?;
  if !status.success() {
    anyhow::bail!("`nft -f {REDIRECT_NFT_PATH}` exited with {status}");
  }
  println!("loaded nftables table dockside_nat (80/443 → 47080/47443 on lo)");

  // Persist via systemd so the rules come back after reboot.
  let unit = format!(
    "[Unit]\n\
     Description=Dockside port redirect (80/443 -> 47080/47443 on loopback)\n\
     After=network-pre.target\n\
     \n\
     [Service]\n\
     Type=oneshot\n\
     RemainAfterExit=yes\n\
     ExecStart={} -f {REDIRECT_NFT_PATH}\n\
     ExecStop={} delete table ip dockside_nat\n\
     \n\
     [Install]\n\
     WantedBy=multi-user.target\n",
    nft.display(),
    nft.display()
  );
  write_root_file(std::path::Path::new(REDIRECT_UNIT_PATH), unit.as_bytes(), 0o644)?;
  println!("wrote {REDIRECT_UNIT_PATH}");

  if let Some(systemctl) = find_tool(&["/usr/bin/systemctl", "/bin/systemctl"]) {
    let _ = std::process::Command::new(&systemctl).arg("daemon-reload").status();
    let status = std::process::Command::new(&systemctl)
      .args(["enable", "--now", REDIRECT_UNIT_NAME])
      .status()?;
    if !status.success() {
      anyhow::bail!("`systemctl enable --now {REDIRECT_UNIT_NAME}` exited with {status}");
    }
    println!("enabled and started {REDIRECT_UNIT_NAME}");
  } else {
    eprintln!(
      "warning: systemctl not found — port redirect is active for this session \
       but will not survive reboot"
    );
  }

  Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_redirect_linux() -> anyhow::Result<()> {
  if let Some(systemctl) = find_tool(&["/usr/bin/systemctl", "/bin/systemctl"]) {
    let _ = std::process::Command::new(&systemctl)
      .args(["disable", "--now", REDIRECT_UNIT_NAME])
      .status();
  }
  if let Some(nft) = find_tool(&["/usr/sbin/nft", "/usr/bin/nft", "/sbin/nft"]) {
    // Best-effort flush; `delete` errors if the table is already gone.
    let _ = std::process::Command::new(nft)
      .args(["delete", "table", "ip", "dockside_nat"])
      .status();
  }
  for path in [REDIRECT_UNIT_PATH, REDIRECT_NFT_PATH] {
    if std::path::Path::new(path).exists() {
      std::fs::remove_file(path)?;
      println!("removed {path}");
    }
  }
  // Best-effort: also revoke any leftover file capability from prior
  // versions of this helper that used `setcap`. Keeps `getcap` clean
  // for users who upgrade.
  if let Some(setcap) = find_tool(&["/usr/sbin/setcap", "/usr/bin/setcap", "/sbin/setcap"])
    && let Ok(exe) = std::env::current_exe()
  {
    let _ = std::process::Command::new(&setcap).args(["-r"]).arg(&exe).status();
  }
  if let Some(systemctl) = find_tool(&["/usr/bin/systemctl", "/bin/systemctl"]) {
    let _ = std::process::Command::new(systemctl).arg("daemon-reload").status();
  }
  Ok(())
}

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
  let path = PathBuf::from(format!("/etc/resolver/{suffix}"));
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
  let path = PathBuf::from(format!("/etc/resolver/{suffix}"));
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
  // Detect the *active* resolver — not just "is the config dir there".
  // Most Arch/Fedora/Ubuntu desktops use systemd-resolved (resolv.conf
  // points at 127.0.0.53); NetworkManager's dnsmasq plugin is opt-in and
  // signaled by `dns=dnsmasq` in NetworkManager.conf.
  if uses_nm_dnsmasq() {
    let path = PathBuf::from(format!("/etc/NetworkManager/dnsmasq.d/dockside-{suffix}.conf"));
    let body = format!("server=/{suffix}/127.0.0.1#{port}\n");
    write_root_file(&path, body.as_bytes(), 0o644)?;
    if let Some(nmcli) = find_tool(&["/usr/bin/nmcli", "/bin/nmcli"]) {
      let _ = std::process::Command::new(nmcli).args(["general", "reload"]).status();
    }
    println!("installed {} (NetworkManager dnsmasq)", path.display());
    return Ok(());
  }

  if uses_systemd_resolved() {
    let dir = Path::new("/etc/systemd/resolved.conf.d");
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("dockside-{suffix}.conf"));
    let body = format!("[Resolve]\nDNS=127.0.0.1:{port}\nDomains=~{suffix}\n");
    write_root_file(&path, body.as_bytes(), 0o644)?;
    if let Some(systemctl) = find_tool(&["/usr/bin/systemctl", "/bin/systemctl"]) {
      let _ = std::process::Command::new(systemctl)
        .args(["restart", "systemd-resolved"])
        .status();
    }
    println!("installed {} (systemd-resolved)", path.display());
    return Ok(());
  }

  anyhow::bail!(
    "no recognised system resolver — need systemd-resolved (active) or \
     NetworkManager with `dns=dnsmasq`. Configure one and re-run, or add \
     a manual resolver entry pointing at 127.0.0.1:{port} for {suffix}."
  );
}

#[cfg(target_os = "linux")]
fn uses_systemd_resolved() -> bool {
  // /etc/resolv.conf points at the systemd-resolved stub (127.0.0.53) when
  // the service is the active resolver. Symlink target is also an
  // acceptable signal (`/run/systemd/resolve/stub-resolv.conf`).
  if let Ok(meta) = std::fs::symlink_metadata("/etc/resolv.conf")
    && meta.file_type().is_symlink()
    && let Ok(target) = std::fs::read_link("/etc/resolv.conf")
  {
    let s = target.to_string_lossy();
    if s.contains("systemd") || s.contains("stub-resolv.conf") {
      return true;
    }
  }
  if let Ok(content) = std::fs::read_to_string("/etc/resolv.conf")
    && content.lines().any(|l| l.trim().starts_with("nameserver 127.0.0.53"))
  {
    return true;
  }
  false
}

#[cfg(target_os = "linux")]
fn uses_nm_dnsmasq() -> bool {
  for path in [
    "/etc/NetworkManager/NetworkManager.conf",
    "/etc/NetworkManager/conf.d/dnsmasq.conf",
  ] {
    if let Ok(content) = std::fs::read_to_string(path)
      && content.lines().any(|l| l.trim() == "dns=dnsmasq")
    {
      return true;
    }
  }
  false
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
  if let Some(systemctl) = find_tool(&["/usr/bin/systemctl", "/bin/systemctl"]) {
    let _ = std::process::Command::new(systemctl)
      .args(["restart", "systemd-resolved"])
      .status();
  }
  if let Some(nmcli) = find_tool(&["/usr/bin/nmcli", "/bin/nmcli"]) {
    let _ = std::process::Command::new(nmcli).args(["general", "reload"]).status();
  }
  Ok(())
}

#[cfg(target_os = "linux")]
fn install_ca_linux(pem_path: &std::path::Path) -> anyhow::Result<()> {
  use std::path::Path;
  // pkexec sanitises PATH; do NOT rely on `which`. Probe absolute paths
  // that match where each distro family installs the trust-store updater.
  if let Some(tool) = find_tool(&[
    "/usr/bin/update-ca-trust",
    "/usr/sbin/update-ca-trust",
    "/sbin/update-ca-trust",
  ]) {
    let dest_dir = Path::new("/etc/ca-certificates/trust-source/anchors");
    std::fs::create_dir_all(dest_dir)?;
    let dest = dest_dir.join("dockside.crt");
    std::fs::copy(pem_path, &dest)?;
    set_world_readable(&dest)?;
    let status = std::process::Command::new(&tool).arg("extract").status()?;
    if !status.success() {
      anyhow::bail!("`{} extract` exited with {status}", tool.display());
    }
    println!("installed CA at {} (Arch/Fedora/RHEL family)", dest.display());
    return Ok(());
  }

  if let Some(tool) = find_tool(&[
    "/usr/sbin/update-ca-certificates",
    "/usr/bin/update-ca-certificates",
    "/sbin/update-ca-certificates",
  ]) {
    let dest_dir = Path::new("/usr/local/share/ca-certificates");
    std::fs::create_dir_all(dest_dir)?;
    let dest = dest_dir.join("dockside.crt");
    std::fs::copy(pem_path, &dest)?;
    set_world_readable(&dest)?;
    let status = std::process::Command::new(&tool).status()?;
    if !status.success() {
      anyhow::bail!("`{}` exited with {status}", tool.display());
    }
    println!("installed CA at {} (Debian/Ubuntu family)", dest.display());
    return Ok(());
  }

  anyhow::bail!(
    "no system trust-store updater found at the usual paths \
     (/usr/bin/update-ca-trust, /usr/sbin/update-ca-certificates) — install \
     the `ca-certificates` package or copy {} into your trust store manually",
    pem_path.display()
  );
}

#[cfg(target_os = "linux")]
fn uninstall_ca_linux() -> anyhow::Result<()> {
  use std::path::Path;
  let mut removed = false;
  for candidate in [
    Path::new("/etc/ca-certificates/trust-source/anchors/dockside.crt"),
    Path::new("/usr/local/share/ca-certificates/dockside.crt"),
  ] {
    if candidate.exists() {
      std::fs::remove_file(candidate)?;
      println!("removed {}", candidate.display());
      removed = true;
    }
  }
  if removed {
    if let Some(tool) = find_tool(&[
      "/usr/bin/update-ca-trust",
      "/usr/sbin/update-ca-trust",
      "/sbin/update-ca-trust",
    ]) {
      let _ = std::process::Command::new(&tool).arg("extract").status();
    }
    if let Some(tool) = find_tool(&[
      "/usr/sbin/update-ca-certificates",
      "/usr/bin/update-ca-certificates",
      "/sbin/update-ca-certificates",
    ]) {
      let _ = std::process::Command::new(&tool).status();
    }
  }
  Ok(())
}

#[cfg(target_os = "linux")]
fn set_world_readable(path: &std::path::Path) -> anyhow::Result<()> {
  use std::os::unix::fs::PermissionsExt;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))?;
  Ok(())
}

#[cfg(target_os = "linux")]
fn find_tool(candidates: &[&str]) -> Option<PathBuf> {
  for path in candidates {
    let p = std::path::Path::new(path);
    if p.is_file() {
      return Some(p.to_path_buf());
    }
  }
  None
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
