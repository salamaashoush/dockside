//! Image vulnerability scanning via Trivy + Dockerfile linting via Hadolint.
//!
//! Wraps the user's local `trivy` and `hadolint` binaries and parses their
//! JSON output. We deliberately don't ship either — if they aren't installed
//! the UI shows a platform-aware install hint.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;
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
    .is_ok_and(|o| o.status.success())
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
  let report: TrivyReport = serde_json::from_str(&stdout).map_err(|e| anyhow!("Failed to parse trivy output: {e}"))?;

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

/// Build a platform-aware install hint for the `hadolint` binary.
pub fn hadolint_install_hint() -> InstallHint {
  let docs_url = "https://github.com/hadolint/hadolint#install";
  #[cfg(target_os = "macos")]
  let commands = vec!["brew install hadolint"];
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
      "arch" | "cachyos" | "manjaro" | "endeavouros" => vec!["sudo pacman -S hadolint"],
      "alpine" => vec!["apk add hadolint"],
      _ => vec![
        "sudo wget -O /usr/local/bin/hadolint https://github.com/hadolint/hadolint/releases/latest/download/hadolint-Linux-x86_64",
        "sudo chmod +x /usr/local/bin/hadolint",
      ],
    }
  };
  #[cfg(target_os = "windows")]
  let commands = vec!["scoop install hadolint", "choco install hadolint"];
  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
  let commands = vec![];

  InstallHint {
    headline: "Hadolint not found in PATH",
    commands,
    docs_url,
  }
}

/// True when `hadolint` is on PATH.
pub fn hadolint_available() -> bool {
  Command::new("hadolint")
    .arg("--version")
    .output()
    .is_ok_and(|o| o.status.success())
}

/// Sentinel error string the UI matches on to render the structured
/// install-hint widget instead of a raw error blob.
pub const ERR_HADOLINT_NOT_INSTALLED: &str = "HADOLINT_NOT_INSTALLED";

/// One Hadolint diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintFinding {
  pub line: u32,
  pub column: u32,
  pub code: String,
  pub level: String,
  pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LintReport {
  pub error: usize,
  pub warning: usize,
  pub info: usize,
  pub style: usize,
  pub findings: Vec<LintFinding>,
}

/// Run `hadolint --format json <path>` and parse the result.
pub fn lint_dockerfile(path: &Path) -> Result<LintReport> {
  if !hadolint_available() {
    return Err(anyhow!(ERR_HADOLINT_NOT_INSTALLED));
  }
  let output = Command::new("hadolint")
    .args(["--format", "json", "--no-fail"])
    .arg(path)
    .output()
    .map_err(|e| anyhow!("Failed to invoke hadolint: {e}"))?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  // Hadolint may exit non-zero when findings exist; only fail on a real
  // run error (no JSON on stdout).
  if stdout.trim().is_empty() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(anyhow!("hadolint produced no output: {stderr}"));
  }
  parse_hadolint_json(&stdout)
}

#[derive(Deserialize, Default)]
struct HadolintEntry {
  line: Option<u32>,
  column: Option<u32>,
  code: Option<String>,
  level: Option<String>,
  message: Option<String>,
}

/// Parse the JSON body of `hadolint --format json` into a `LintReport`.
/// Pulled out of `lint_dockerfile` so we can unit-test the parsing
/// logic without spawning the binary.
pub(crate) fn parse_hadolint_json(stdout: &str) -> Result<LintReport> {
  let raw: Vec<HadolintEntry> =
    serde_json::from_str(stdout).map_err(|e| anyhow!("Failed to parse hadolint output: {e}"))?;
  let mut report = LintReport::default();
  for entry in raw {
    let level = entry.level.unwrap_or_else(|| "warning".to_string());
    match level.as_str() {
      "error" => report.error += 1,
      "info" => report.info += 1,
      "style" => report.style += 1,
      _ => report.warning += 1,
    }
    report.findings.push(LintFinding {
      line: entry.line.unwrap_or(0),
      column: entry.column.unwrap_or(0),
      code: entry.code.unwrap_or_default(),
      level,
      message: entry.message.unwrap_or_default(),
    });
  }
  report.findings.sort_by(|a, b| {
    let weight = |s: &str| match s {
      "error" => 0,
      "warning" => 1,
      "info" => 2,
      "style" => 3,
      _ => 4,
    };
    weight(a.level.as_str())
      .cmp(&weight(b.level.as_str()))
      .then_with(|| a.line.cmp(&b.line))
  });
  Ok(report)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_empty_hadolint() {
    let r = parse_hadolint_json("[]").unwrap();
    assert_eq!(r.findings.len(), 0);
    assert_eq!(r.error + r.warning + r.info + r.style, 0);
  }

  #[test]
  fn parse_mixed_levels_and_sort() {
    let json = r#"[
      {"line":10,"column":1,"code":"DL3008","level":"warning","message":"Pin versions"},
      {"line":3,"column":1,"code":"DL3001","level":"error","message":"For some bash command"},
      {"line":7,"column":1,"code":"SC2086","level":"info","message":"Quote variable"},
      {"line":2,"column":1,"code":"DL4006","level":"style","message":"Set the SHELL option"}
    ]"#;
    let r = parse_hadolint_json(json).unwrap();
    assert_eq!(r.error, 1);
    assert_eq!(r.warning, 1);
    assert_eq!(r.info, 1);
    assert_eq!(r.style, 1);
    // error (DL3001) sorts before warning (DL3008).
    assert_eq!(r.findings[0].code, "DL3001");
    assert_eq!(r.findings[1].code, "DL3008");
    assert_eq!(r.findings[2].code, "SC2086");
    assert_eq!(r.findings[3].code, "DL4006");
  }

  #[test]
  fn parse_unknown_level_buckets_as_warning() {
    let json = r#"[{"line":1,"column":1,"code":"X","level":"weird","message":"?"}]"#;
    let r = parse_hadolint_json(json).unwrap();
    assert_eq!(r.warning, 1);
    assert_eq!(r.findings[0].level, "weird");
  }

  #[test]
  fn parse_missing_fields_defaults() {
    let json = r#"[{"code":"DL3000","message":"hi"}]"#;
    let r = parse_hadolint_json(json).unwrap();
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].line, 0);
    assert_eq!(r.findings[0].level, "warning");
    assert_eq!(r.warning, 1);
  }

  #[test]
  fn parse_invalid_json_errors() {
    assert!(parse_hadolint_json("not json").is_err());
  }
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
