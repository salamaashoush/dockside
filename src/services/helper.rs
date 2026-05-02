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

/// Locate the `dockside-helper` binary. Looks first next to the running
/// `dockside` executable, then on `PATH`.
pub fn helper_path() -> Result<PathBuf> {
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

/// Invoke the helper with `args`, prompting the user for elevation in the
/// platform-native way. Returns Ok on success, error with combined stderr
/// otherwise.
pub fn run_privileged(args: &[&str]) -> Result<String> {
  let helper = helper_path()?;
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
