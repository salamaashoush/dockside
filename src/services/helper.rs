//! Privileged-helper invocation wrapper. The main app calls into this
//! module instead of shelling out directly so we keep the auth-prompt
//! plumbing in one place.
//!
//! macOS: wrap the helper call in an `osascript` "do shell script ... with
//! administrator privileges" so the user gets the standard Touch ID/password
//! dialog. Pattern lifted from mkcert.
//!
//! Linux: invoke through `pkexec` with a `PolicyKit` `.policy` file
//! installed into `/usr/share/polkit-1/actions/` (shipped with the
//! distribution package). Pattern lifted from `NetworkManager` + Tailscale.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

/// Probe the helper version (no privilege required). Bails when the
/// system-installed helper is older than the running app, which happens
/// during iterative development after every `cargo build` of new helper
/// subcommands.
fn require_matching_helper_version(helper: &std::path::Path) -> Result<()> {
  let app_version = env!("CARGO_PKG_VERSION");
  let output = Command::new(helper)
    .arg("version")
    .output()
    .with_context(|| format!("run {} version", helper.display()))?;
  if !output.status.success() {
    // Old helpers without a `version` subcommand exit with usage code 64.
    // Treat that as "stale" so we surface a refresh hint.
    anyhow::bail!(
      "helper at {} does not understand `version` (likely older than this app)",
      helper.display()
    );
  }
  let stdout = String::from_utf8_lossy(&output.stdout);
  // Format: "dockside-helper <version>"
  let helper_version = stdout.split_whitespace().nth(1).map_or("", str::trim);
  if helper_version != app_version {
    anyhow::bail!(
      "helper at {} reports version {helper_version:?}; app is {app_version}",
      helper.display()
    );
  }
  Ok(())
}

/// Locate the `dockside-helper` binary. The authoritative lookup on
/// Linux reads the polkit policy file and returns whatever path it
/// annotates — that's the binary pkexec will run under cached
/// `auth_admin_keep`, so the app must call exactly that path or the
/// auth dialog reappears every invocation. When the policy file is
/// missing (bootstrap not run yet) we fall through to filesystem
/// probes covering the canonical install layouts and the dev sibling
/// case.
pub fn helper_path() -> Result<PathBuf> {
  if let Some(p) = polkit_helper_path()
    && p.is_file()
  {
    return Ok(p);
  }
  #[cfg(unix)]
  for candidate in [
    "/usr/local/libexec/dockside-helper",
    "/usr/libexec/dockside-helper",
    "/usr/bin/dockside-helper",
  ] {
    let p = PathBuf::from(candidate);
    if p.is_file() {
      return Ok(p);
    }
  }
  let exe = std::env::current_exe().context("locate current_exe")?;
  if let Some(parent) = exe.parent() {
    let candidate = parent.join(if cfg!(windows) {
      "dockside-helper.exe"
    } else {
      "dockside-helper"
    });
    if candidate.is_file() {
      return Ok(candidate);
    }
  }
  which::which("dockside-helper").context("dockside-helper not found on PATH")
}

/// Parse `/usr/share/polkit-1/actions/dev.dockside.helper.policy` and
/// return the binary path declared in the
/// `org.freedesktop.policykit.exec.path` annotation. Returns `None` on
/// non-Linux and when the policy file is missing or malformed.
fn polkit_helper_path() -> Option<PathBuf> {
  const NEEDLE: &str = "org.freedesktop.policykit.exec.path";
  #[cfg(target_os = "linux")]
  {
    let policy = std::fs::read_to_string("/usr/share/polkit-1/actions/dev.dockside.helper.policy").ok()?;
    let idx = policy.find(NEEDLE)?;
    let after = &policy[idx + NEEDLE.len()..];
    let start = after.find('>')? + 1;
    let end_rel = after[start..].find('<')?;
    let path = after[start..start + end_rel].trim();
    if path.is_empty() {
      None
    } else {
      Some(PathBuf::from(path))
    }
  }
  #[cfg(not(target_os = "linux"))]
  {
    let _ = NEEDLE;
    None
  }
}

/// Path to the helper binary that ships next to the running `dockside`
/// executable. Used by `bootstrap` to upgrade the system-installed copy:
/// invoking the system helper would copy *itself* (stale) over itself,
/// so we always pkexec the fresh sibling for the install/upgrade step.
pub fn sibling_helper_path() -> Result<PathBuf> {
  let exe = std::env::current_exe().context("locate current_exe")?;
  let parent = exe
    .parent()
    .ok_or_else(|| anyhow::anyhow!("current_exe has no parent dir"))?;
  let candidate = parent.join(if cfg!(windows) {
    "dockside-helper.exe"
  } else {
    "dockside-helper"
  });
  if candidate.is_file() {
    Ok(candidate)
  } else {
    anyhow::bail!("dockside-helper not found next to {}", exe.display());
  }
}

/// Invoke the helper with `args`, prompting the user for elevation in the
/// platform-native way. Returns Ok on success, error with combined stderr
/// otherwise.
pub fn run_privileged(args: &[&str]) -> Result<String> {
  let helper = helper_path()?;
  // System helper at /usr/local/libexec is installed by `bootstrap` and
  // can drift from the in-tree binary as you iterate. Refuse to invoke a
  // helper older than the running app — surface a clear message so the
  // user knows to re-run setup, instead of getting cryptic "unknown
  // subcommand" errors from pkexec.
  if let Err(e) = require_matching_helper_version(&helper) {
    anyhow::bail!("{e} — click \"Re-run setup\" in Settings → Local DNS to refresh.");
  }
  run_privileged_with(&helper, args)
}

/// Like `run_privileged` but pkexec's a specific helper binary, skipping
/// the system-path lookup and version check. Used by `bootstrap` to
/// invoke the fresh in-tree helper so the upgrade actually happens (the
/// stale system helper would otherwise copy itself to itself).
pub fn run_privileged_with(helper: &std::path::Path, args: &[&str]) -> Result<String> {
  let helper_str = helper
    .to_str()
    .ok_or_else(|| anyhow::anyhow!("helper path is not valid UTF-8"))?;
  tracing::info!("dns helper: invoking {helper_str:?} {args:?}");

  #[cfg(target_os = "macos")]
  {
    macos_run(helper_str, args)
  }
  #[cfg(target_os = "linux")]
  {
    linux_run(helper_str, args)
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = (helper_str, args);
    anyhow::bail!("privileged helper not implemented for this OS yet");
  }
}

#[cfg(target_os = "macos")]
fn macos_run(helper_path: &str, args: &[&str]) -> Result<String> {
  // Build a single shell command string: helper_path "arg1" "arg2" ...
  let escaped_args: String = args
    .iter()
    .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
    .collect::<Vec<_>>()
    .join(" ");
  let escaped_helper = helper_path.replace('"', "\\\"");
  let shell = format!("\"{escaped_helper}\" {escaped_args}");
  let script = format!(
    "do shell script \"{}\" with administrator privileges",
    shell.replace('"', "\\\"")
  );
  let output = Command::new("osascript")
    .args(["-e", &script])
    .output()
    .context("invoke osascript")?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    anyhow::bail!("helper failed (osascript): {stderr}");
  }
  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "linux")]
fn linux_run(helper_path: &str, args: &[&str]) -> Result<String> {
  let mut cmd = Command::new("pkexec");
  cmd.arg(helper_path);
  for a in args {
    cmd.arg(a);
  }
  let output = cmd.output().context("invoke pkexec")?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    anyhow::bail!("helper failed (pkexec exit {}): {stderr}", output.status);
  }
  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
