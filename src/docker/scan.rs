//! Image vulnerability scanning via Trivy.
//!
//! Wraps the user's local `trivy` binary (https://aquasecurity.github.io/trivy)
//! and parses its JSON output. We deliberately don't ship Trivy — if it isn't
//! installed the UI shows an install hint.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanSummary {
  pub critical: usize,
  pub high: usize,
  pub medium: usize,
  pub low: usize,
  pub unknown: usize,
  pub vulns: Vec<Vulnerability>,
}

impl ScanSummary {
  pub fn total(&self) -> usize {
    self.critical + self.high + self.medium + self.low + self.unknown
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
  pub id: String,
  pub severity: String,
  pub package: String,
  pub installed_version: String,
  pub fixed_version: Option<String>,
  pub title: String,
  pub primary_url: Option<String>,
}

/// Structured install guidance for surfacing in the UI: a short
/// headline, a list of copy-able shell commands relevant to the host
/// platform, plus a docs URL.
#[derive(Debug, Clone)]
pub struct InstallHint {
  pub headline: &'static str,
  pub commands: Vec<&'static str>,
  pub docs_url: &'static str,
}

/// Build a platform-aware install hint for the `trivy` binary.
pub fn trivy_install_hint() -> InstallHint {
  let docs_url = "https://aquasecurity.github.io/trivy/latest/getting-started/installation/";
  #[cfg(target_os = "macos")]
  let commands = vec!["brew install trivy"];
  #[cfg(target_os = "linux")]
  let commands: Vec<&'static str> = {
    let id = std::fs::read_to_string("/etc/os-release")
      .ok()
      .and_then(|s| {
        s.lines()
          .find_map(|l| l.strip_prefix("ID=").map(|v| v.trim_matches('"').to_lowercase()))
      })
      .unwrap_or_default();
    match id.as_str() {
      "arch" | "cachyos" | "manjaro" | "endeavouros" => vec!["sudo pacman -S trivy"],
      "fedora" | "rhel" | "centos" => vec!["sudo dnf install trivy"],
      "alpine" => vec!["apk add trivy"],
      "debian" | "ubuntu" | "pop" | "linuxmint" => vec![
        "sudo apt-get install wget gnupg",
        "wget -qO - https://aquasecurity.github.io/trivy-repo/deb/public.key | sudo apt-key add -",
        "echo \"deb https://aquasecurity.github.io/trivy-repo/deb $(lsb_release -sc) main\" | sudo tee -a /etc/apt/sources.list.d/trivy.list",
        "sudo apt-get update && sudo apt-get install trivy",
      ],
      _ => vec![
        "sudo apt install trivy   # Debian/Ubuntu",
        "sudo pacman -S trivy     # Arch",
        "sudo dnf install trivy   # Fedora",
      ],
    }
  };
  #[cfg(target_os = "windows")]
  let commands = vec!["scoop install trivy", "choco install trivy"];
  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
  let commands = vec![];

  InstallHint {
    headline: "Trivy not found in PATH",
    commands,
    docs_url,
  }
}

/// True when `trivy` is on PATH.
pub fn trivy_available() -> bool {
  Command::new("trivy")
    .arg("--version")
    .output()
    .map(|o| o.status.success())
    .unwrap_or(false)
}

/// Sentinel error string the UI matches on to render the structured
/// install-hint widget instead of a raw error blob.
pub const ERR_TRIVY_NOT_INSTALLED: &str = "TRIVY_NOT_INSTALLED";

/// Run `trivy image --format json --quiet <id>` and parse the result.
pub fn scan_image(image_ref: &str) -> Result<ScanSummary> {
  if !trivy_available() {
    return Err(anyhow!(ERR_TRIVY_NOT_INSTALLED));
  }
  let output = Command::new("trivy")
    .args(["image", "--format", "json", "--quiet", "--scanners", "vuln", image_ref])
    .output()
    .map_err(|e| anyhow!("Failed to invoke trivy: {e}"))?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(anyhow!("trivy exited non-zero: {stderr}"));
  }

  let stdout = String::from_utf8_lossy(&output.stdout);
  let report: TrivyReport =
    serde_json::from_str(&stdout).map_err(|e| anyhow!("Failed to parse trivy output: {e}"))?;

  let mut summary = ScanSummary::default();
  for result in report.results.unwrap_or_default() {
    for v in result.vulnerabilities.unwrap_or_default() {
      let severity = v.severity.unwrap_or_else(|| "UNKNOWN".to_string());
      match severity.as_str() {
        "CRITICAL" => summary.critical += 1,
        "HIGH" => summary.high += 1,
        "MEDIUM" => summary.medium += 1,
        "LOW" => summary.low += 1,
        _ => summary.unknown += 1,
      }
      summary.vulns.push(Vulnerability {
        id: v.vulnerability_id.unwrap_or_default(),
        severity,
        package: v.pkg_name.unwrap_or_default(),
        installed_version: v.installed_version.unwrap_or_default(),
        fixed_version: v.fixed_version,
        title: v.title.unwrap_or_default(),
        primary_url: v.primary_url,
      });
    }
  }

  // Sort by severity (critical first) then CVE id for stable display.
  summary.vulns.sort_by(|a, b| {
    let weight = |s: &str| match s {
      "CRITICAL" => 0,
      "HIGH" => 1,
      "MEDIUM" => 2,
      "LOW" => 3,
      _ => 4,
    };
    weight(a.severity.as_str())
      .cmp(&weight(b.severity.as_str()))
      .then_with(|| a.id.cmp(&b.id))
  });

  Ok(summary)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct TrivyReport {
  #[allow(dead_code)]
  schema_version: Option<u32>,
  results: Option<Vec<TrivyResult>>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct TrivyResult {
  vulnerabilities: Option<Vec<TrivyVuln>>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct TrivyVuln {
  #[serde(rename = "VulnerabilityID")]
  vulnerability_id: Option<String>,
  pkg_name: Option<String>,
  installed_version: Option<String>,
  fixed_version: Option<String>,
  severity: Option<String>,
  title: Option<String>,
  primary_url: Option<String>,
}
