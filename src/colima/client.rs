use anyhow::{Result, anyhow};
use std::process::Stdio;

use super::{ColimaConfig, ColimaVm, MountType, VmArch, VmFileEntry, VmOsInfo, VmRuntime, VmStatus, VmType};
use crate::utils::colima_cmd;

pub struct ColimaClient;

impl ColimaClient {
  pub fn new() -> Self {
    Self
  }

  /// Get colima version
  pub fn version() -> Result<String> {
    let output = colima_cmd()
      .arg("version")
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .output()?;

    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout);
      Ok(stdout.trim().to_string())
    } else {
      Err(anyhow!("Failed to get colima version"))
    }
  }

  /// List all Colima VMs
  pub fn list() -> Result<Vec<ColimaVm>> {
    let cmd = colima_cmd();
    tracing::debug!("Running colima list with command: {:?}", cmd.get_program());

    let output = colima_cmd()
      .args(["list", "--json"])
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .output()?;

    let mut vms = Vec::new();

    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout);
      tracing::debug!("colima list output: {}", stdout);
      for line in stdout.lines() {
        if line.trim().is_empty() {
          continue;
        }
        if let Ok(mut vm) = Self::parse_list_json(line) {
          // If VM is running, get detailed status
          if vm.status.is_running()
            && let Ok(detailed) = Self::status(Some(&vm.name))
          {
            vm = detailed;
            vm.status = VmStatus::Running;
          }
          vms.push(vm);
        }
      }
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr);
      tracing::warn!("colima list failed with status {:?}: {}", output.status, stderr);
    }

    tracing::info!("Found {} Colima VMs", vms.len());

    // If no VMs found, check if default profile exists (might be stopped)
    if vms.is_empty()
      && let Ok(default_vm) = Self::status(None)
    {
      tracing::debug!("No VMs from list, found default profile via status");
      vms.push(default_vm);
    }

    Ok(vms)
  }

  fn parse_list_json(json_str: &str) -> Result<ColimaVm> {
    let value: serde_json::Value = serde_json::from_str(json_str)?;

    let name = value["name"].as_str().unwrap_or("default").to_string();

    let status = match value["status"].as_str() {
      Some("Running") => VmStatus::Running,
      Some("Stopped") => VmStatus::Stopped,
      _ => VmStatus::Unknown,
    };

    let runtime = match value["runtime"].as_str() {
      Some("containerd") => VmRuntime::Containerd,
      Some("incus") => VmRuntime::Incus,
      _ => VmRuntime::Docker,
    };

    let arch = match value["arch"].as_str() {
      Some("x86_64") => VmArch::X86_64,
      _ => VmArch::Aarch64,
    };

    let cpus = u32::try_from(value["cpus"].as_u64().or_else(|| value["cpu"].as_u64()).unwrap_or(2)).unwrap_or(2);
    let memory = value["memory"].as_u64().unwrap_or(2 * 1024 * 1024 * 1024);
    let disk = value["disk"].as_u64().unwrap_or(60 * 1024 * 1024 * 1024);
    // Use kubernetes field directly - note: colima list may have stale data
    // For running VMs, we get accurate data from colima status
    let kubernetes = value["kubernetes"].as_bool().unwrap_or(false);
    let address = value["address"].as_str().map(ToString::to_string);

    Ok(ColimaVm {
      name,
      status,
      runtime,
      arch,
      cpus,
      memory,
      disk,
      kubernetes,
      address,
      ..Default::default()
    })
  }

  fn parse_status_json(json_str: &str, profile_name: Option<&str>) -> Result<ColimaVm> {
    let value: serde_json::Value = serde_json::from_str(json_str)?;

    // Use the profile_name we were called with, falling back to parsing display_name
    let name = if let Some(p) = profile_name {
      p.to_string()
    } else {
      // Try to extract from display_name like "colima [profile=salama]" -> "salama"
      // or just use "default"
      value["display_name"]
        .as_str()
        .and_then(|s| {
          if s.contains("[profile=") {
            s.split("[profile=")
              .nth(1)
              .and_then(|p| p.strip_suffix(']'))
              .map(ToString::to_string)
          } else {
            None
          }
        })
        .or_else(|| value["name"].as_str().map(ToString::to_string))
        .unwrap_or_else(|| "default".to_string())
    };

    let runtime = match value["runtime"].as_str() {
      Some("containerd") => VmRuntime::Containerd,
      Some("incus") => VmRuntime::Incus,
      _ => VmRuntime::Docker,
    };

    let arch = match value["arch"].as_str() {
      Some("x86_64") => VmArch::X86_64,
      _ => VmArch::Aarch64,
    };

    let vm_type = match value["driver"].as_str() {
      Some(d) if d.contains("QEMU") => Some(VmType::Qemu),
      Some(d) if d.contains("Virtualization") => Some(VmType::Vz),
      Some(d) if d.to_lowercase().contains("krunkit") || d.to_lowercase().contains("libkrun") => Some(VmType::Krunkit),
      _ => None,
    };

    let mount_type = match value["mount_type"].as_str() {
      Some("sshfs") => Some(MountType::Sshfs),
      Some("9p") => Some(MountType::NineP),
      Some("virtiofs") => Some(MountType::Virtiofs),
      _ => None,
    };

    let cpus = u32::try_from(value["cpu"].as_u64().or_else(|| value["cpus"].as_u64()).unwrap_or(2)).unwrap_or(2);
    let memory = value["memory"].as_u64().unwrap_or(2 * 1024 * 1024 * 1024);
    let disk = value["disk"].as_u64().unwrap_or(60 * 1024 * 1024 * 1024);
    // Use kubernetes field directly from colima status --json (most reliable source)
    let kubernetes = value["kubernetes"].as_bool().unwrap_or(false);

    let docker_socket = value["docker_socket"].as_str().map(ToString::to_string);
    let containerd_socket = value["containerd_socket"].as_str().map(ToString::to_string);
    let driver = value["driver"].as_str().map(ToString::to_string);

    Ok(ColimaVm {
      name,
      status: VmStatus::Running, // status command only works on running VMs
      runtime,
      arch,
      cpus,
      memory,
      disk,
      kubernetes,
      address: None,
      driver,
      vm_type,
      mount_type,
      docker_socket,
      containerd_socket,
      hostname: None,
      rosetta: false,
      ssh_agent: false,
    })
  }

  /// Get detailed status of a VM (only works when running)
  pub fn status(name: Option<&str>) -> Result<ColimaVm> {
    let mut cmd = colima_cmd();
    cmd.arg("status").arg("--json");

    if let Some(n) = name
      && n != "default"
    {
      cmd.arg("--profile").arg(n);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      if stderr.contains("not running") || stderr.contains("does not exist") {
        return Ok(ColimaVm {
          name: name.unwrap_or("default").to_string(),
          status: VmStatus::Stopped,
          ..Default::default()
        });
      }
      return Err(anyhow!("colima status failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Self::parse_status_json(&stdout, name)
  }

  /// Start a VM using config file approach
  /// This writes the config to the YAML file then starts colima
  pub fn start_with_config(profile: &str, config: &ColimaConfig) -> Result<()> {
    // Write config to the profile's config file
    let profile_opt = if profile == "default" { None } else { Some(profile) };
    Self::write_config(profile_opt, config)?;

    // Start with the profile (colima will read from config file)
    let mut cmd = colima_cmd();
    cmd.arg("start");

    if profile != "default" {
      cmd.arg("--profile").arg(profile);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(anyhow!("colima start failed: {stderr}"));
    }

    Ok(())
  }

  /// Start an existing VM (uses its existing config file)
  pub fn start_existing(name: Option<&str>) -> Result<()> {
    let mut cmd = colima_cmd();
    cmd.arg("start");

    if let Some(n) = name
      && n != "default"
    {
      cmd.arg("--profile").arg(n);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(anyhow!("colima start failed: {stderr}"));
    }

    Ok(())
  }

  /// Stop a VM
  pub fn stop(name: Option<&str>) -> Result<()> {
    let mut cmd = colima_cmd();
    cmd.arg("stop");

    if let Some(n) = name
      && n != "default"
    {
      cmd.arg("--profile").arg(n);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(anyhow!("colima stop failed: {stderr}"));
    }

    Ok(())
  }

  /// Restart a VM
  pub fn restart(name: Option<&str>) -> Result<()> {
    let mut cmd = colima_cmd();
    cmd.arg("restart");

    if let Some(n) = name
      && n != "default"
    {
      cmd.arg("--profile").arg(n);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(anyhow!("colima restart failed: {stderr}"));
    }

    Ok(())
  }

  /// Delete a VM
  pub fn delete(name: Option<&str>, force: bool) -> Result<()> {
    let mut cmd = colima_cmd();
    cmd.arg("delete");

    if let Some(n) = name
      && n != "default"
    {
      cmd.arg("--profile").arg(n);
    }

    if force {
      cmd.arg("--force");
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(anyhow!("colima delete failed: {stderr}"));
    }

    Ok(())
  }

  /// Get the docker socket path for a VM
  #[allow(dead_code)]
  pub fn socket_path(name: Option<&str>) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    let profile = name.unwrap_or("default");
    format!("{}/.colima/{}/docker.sock", home.display(), profile)
  }

  /// Run a command in the VM via SSH
  pub fn run_command(name: Option<&str>, command: &str) -> Result<String> {
    let mut cmd = colima_cmd();
    cmd.arg("ssh");

    if let Some(n) = name
      && n != "default"
    {
      cmd.arg("--profile").arg(n);
    }

    cmd.arg("--").arg("sh").arg("-c").arg(command);

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if output.status.success() {
      Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr);
      Err(anyhow!("Command failed: {stderr}"))
    }
  }

  /// Get OS information from the VM
  pub fn get_os_info(name: Option<&str>) -> Result<VmOsInfo> {
    // Get OS release info
    let os_release = Self::run_command(name, "cat /etc/os-release 2>/dev/null || echo ''")?;

    // Get kernel info
    let kernel = Self::run_command(name, "uname -r 2>/dev/null || echo 'unknown'")?;

    // Get architecture
    let arch = Self::run_command(name, "uname -m 2>/dev/null || echo 'unknown'")?;

    let mut info = VmOsInfo {
      kernel: kernel.trim().to_string(),
      arch: arch.trim().to_string(),
      ..Default::default()
    };

    // Parse os-release
    for line in os_release.lines() {
      if let Some((key, value)) = line.split_once('=') {
        let value = value.trim_matches('"');
        match key {
          "PRETTY_NAME" => info.pretty_name = value.to_string(),
          "NAME" => info.name = value.to_string(),
          "VERSION" => info.version = value.to_string(),
          "VERSION_ID" => info.version_id = value.to_string(),
          "ID" => info.id = value.to_string(),
          _ => {}
        }
      }
    }

    Ok(info)
  }

  /// Get system logs from the VM
  pub fn get_system_logs(name: Option<&str>, lines: u32) -> Result<String> {
    Self::run_command(
      name,
      &format!("sudo journalctl --no-pager -n {lines} 2>/dev/null || echo 'Unable to fetch logs'"),
    )
  }

  /// Get Docker service logs from the VM
  pub fn get_docker_logs(name: Option<&str>, lines: u32) -> Result<String> {
    Self::run_command(
      name,
      &format!("sudo journalctl -u docker --no-pager -n {lines} 2>/dev/null || echo 'Unable to fetch Docker logs'"),
    )
  }

  /// Get containerd service logs from the VM
  pub fn get_containerd_logs(name: Option<&str>, lines: u32) -> Result<String> {
    Self::run_command(
      name,
      &format!(
        "sudo journalctl -u containerd --no-pager -n {lines} 2>/dev/null || echo 'Unable to fetch containerd logs'"
      ),
    )
  }

  /// List files in a directory in the VM
  pub fn list_files(name: Option<&str>, path: &str) -> Result<Vec<VmFileEntry>> {
    // Use ls -la with specific format for parsing
    let output = Self::run_command(
      name,
      &format!("ls -la --time-style=long-iso {path} 2>/dev/null || ls -la {path}"),
    )?;

    let mut entries = Vec::new();

    for line in output.lines().skip(1) {
      // Skip "total" line
      if line.starts_with("total") {
        continue;
      }

      let parts: Vec<&str> = line.split_whitespace().collect();
      if parts.len() >= 8 {
        let permissions = parts[0].to_string();
        let owner = (*parts.get(2).unwrap_or(&"")).to_string();
        let size: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);

        // Date can be in different formats
        let modified = if parts.len() >= 8 {
          format!("{} {}", parts.get(5).unwrap_or(&""), parts.get(6).unwrap_or(&""))
        } else {
          String::new()
        };

        // Name is the last part (may contain spaces for symlinks)
        let name_start = if parts.len() >= 8 { 7 } else { parts.len() - 1 };
        let file_name = parts[name_start..].join(" ");

        // Handle symlinks (name -> target)
        let (display_name, is_symlink) = if file_name.contains(" -> ") {
          let n = file_name.split(" -> ").next().unwrap_or(&file_name);
          (n.to_string(), true)
        } else {
          (file_name.clone(), permissions.starts_with('l'))
        };

        // Skip . and .. entries
        if display_name == "." || display_name == ".." {
          continue;
        }

        let is_dir = permissions.starts_with('d');
        let full_path = if path == "/" {
          format!("/{display_name}")
        } else {
          format!("{}/{}", path.trim_end_matches('/'), display_name)
        };

        entries.push(VmFileEntry {
          name: display_name,
          path: full_path,
          is_dir,
          is_symlink,
          size,
          permissions,
          owner,
          modified,
        });
      }
    }

    // Sort: directories first, then by name
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
      (true, false) => std::cmp::Ordering::Less,
      (false, true) => std::cmp::Ordering::Greater,
      _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
  }

  /// Read a file from the VM
  pub fn read_file(name: Option<&str>, path: &str, max_lines: u32) -> Result<String> {
    Self::run_command(
      name,
      &format!("head -n {max_lines} {path} 2>/dev/null || echo 'Unable to read file'"),
    )
  }

  /// Resolve a symlink to its target path
  pub fn resolve_symlink(name: Option<&str>, path: &str) -> Result<String> {
    let output = Self::run_command(name, &format!("readlink -f {path} 2>/dev/null"))?;
    Ok(output.trim().to_string())
  }

  /// Check if a path is a directory
  pub fn is_directory(name: Option<&str>, path: &str) -> Result<bool> {
    let output = Self::run_command(name, &format!("test -d '{path}' && echo dir"))?;
    Ok(output.contains("dir"))
  }

  /// Get disk usage info from the VM
  pub fn get_disk_usage(name: Option<&str>) -> Result<String> {
    Self::run_command(name, "df -h / 2>/dev/null || echo 'Unable to get disk usage'")
  }

  /// Get memory info from the VM
  pub fn get_memory_info(name: Option<&str>) -> Result<String> {
    Self::run_command(name, "free -h 2>/dev/null || echo 'Unable to get memory info'")
  }

  /// Get running processes from the VM
  pub fn get_processes(name: Option<&str>) -> Result<String> {
    Self::run_command(
      name,
      "ps aux --sort=-%mem 2>/dev/null | head -20 || echo 'Unable to get processes'",
    )
  }

  /// Get the config file path for a profile
  pub fn config_path(name: Option<&str>) -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    let profile = name.unwrap_or("default");
    home.join(".colima").join(profile).join("colima.yaml")
  }

  /// Read the configuration for a profile
  pub fn read_config(name: Option<&str>) -> Result<ColimaConfig> {
    let path = Self::config_path(name);
    if !path.exists() {
      return Ok(ColimaConfig::default());
    }

    let content =
      std::fs::read_to_string(&path).map_err(|e| anyhow!("Failed to read config at {}: {e}", path.display()))?;

    serde_yaml::from_str(&content).map_err(|e| anyhow!("Failed to parse config: {e}"))
  }

  /// Write the configuration for a profile
  pub fn write_config(name: Option<&str>, config: &ColimaConfig) -> Result<()> {
    let path = Self::config_path(name);

    // Ensure directory exists
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }

    let content = serde_yaml::to_string(config)?;
    std::fs::write(&path, content).map_err(|e| anyhow!("Failed to write config at {}: {e}", path.display()))
  }

  /// Update the container runtime in the VM
  /// This updates Docker/containerd to the latest version
  pub fn update(name: Option<&str>) -> Result<()> {
    let mut cmd = colima_cmd();
    cmd.arg("update");

    if let Some(n) = name
      && n != "default"
    {
      cmd.arg("--profile").arg(n);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(anyhow!("colima update failed: {stderr}"));
    }

    Ok(())
  }

  /// Prune cached downloaded assets (VM images, etc.)
  pub fn prune(all: bool, force: bool) -> Result<String> {
    let mut cmd = colima_cmd();
    cmd.arg("prune");

    if all {
      cmd.arg("--all");
    }

    if force {
      cmd.arg("--force");
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(anyhow!("colima prune failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
  }

  /// Get the SSH configuration for a profile
  /// Returns the SSH config that can be added to ~/.ssh/config
  pub fn ssh_config(name: Option<&str>) -> Result<String> {
    let mut cmd = colima_cmd();
    cmd.arg("ssh-config");

    if let Some(n) = name
      && n != "default"
    {
      cmd.arg("--profile").arg(n);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(anyhow!("colima ssh-config failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
  }

  /// Get the cache directory size
  pub fn cache_size() -> Result<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let cache_path = home.join(".colima").join("_wrapper");

    if !cache_path.exists() {
      return Ok("0 B".to_string());
    }

    // Use du to get directory size
    let output = std::process::Command::new("du")
      .args(["-sh", &cache_path.to_string_lossy()])
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .output()?;

    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout);
      // Format: "123M\t/path/to/dir"
      if let Some(size) = stdout.split_whitespace().next() {
        return Ok(size.to_string());
      }
    }

    Ok("Unknown".to_string())
  }

  /// Run a provision script in the VM
  pub fn run_provision_script(name: Option<&str>, script: &str, as_root: bool) -> Result<String> {
    let command = if as_root {
      format!("sudo sh -c '{}'", script.replace('\'', "'\"'\"'"))
    } else {
      format!("sh -c '{}'", script.replace('\'', "'\"'\"'"))
    };

    Self::run_command(name, &command)
  }

  /// Get the path to the default template file
  pub fn template_path() -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".colima").join("_templates").join("default.yaml")
  }

  /// Read the default template content
  /// First tries to read from ~/.colima/_templates/default.yaml
  /// Falls back to `colima template --print` if file doesn't exist
  pub fn read_template() -> Result<String> {
    let path = Self::template_path();

    // Try reading from file first
    if path.exists()
      && let Ok(content) = std::fs::read_to_string(&path)
    {
      return Ok(content);
    }

    // Fall back to colima template --print
    let mut cmd = colima_cmd();
    cmd.args(["template", "--print"]);
    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

    if output.status.success() {
      Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
      Err(anyhow!("Failed to read template"))
    }
  }

  /// Write the default template content
  pub fn write_template(content: &str) -> Result<()> {
    let path = Self::template_path();
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content).map_err(|e| anyhow!("Failed to write template: {e}"))
  }
}

impl Default for ColimaClient {
  fn default() -> Self {
    Self::new()
  }
}
