use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VmStatus {
  Running,
  Stopped,
  #[default]
  Unknown,
}

impl VmStatus {
  pub fn is_running(&self) -> bool {
    matches!(self, VmStatus::Running)
  }
}

impl std::fmt::Display for VmStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VmStatus::Running => write!(f, "Running"),
      VmStatus::Stopped => write!(f, "Stopped"),
      VmStatus::Unknown => write!(f, "Unknown"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VmRuntime {
  #[default]
  Docker,
  Containerd,
  Incus,
}

impl std::fmt::Display for VmRuntime {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VmRuntime::Docker => write!(f, "docker"),
      VmRuntime::Containerd => write!(f, "containerd"),
      VmRuntime::Incus => write!(f, "incus"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VmArch {
  #[default]
  Aarch64,
  X86_64,
}

impl VmArch {
  pub fn display_name(&self) -> &'static str {
    match self {
      VmArch::Aarch64 => "ARM64 (aarch64)",
      VmArch::X86_64 => "x86_64",
    }
  }
}

impl std::fmt::Display for VmArch {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VmArch::Aarch64 => write!(f, "aarch64"),
      VmArch::X86_64 => write!(f, "x86_64"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VmType {
  #[default]
  Vz,
  Qemu,
}

impl std::fmt::Display for VmType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VmType::Vz => write!(f, "vz"),
      VmType::Qemu => write!(f, "qemu"),
    }
  }
}

impl VmType {
  pub fn display_name(&self) -> &'static str {
    match self {
      VmType::Vz => "macOS Virtualization.Framework",
      VmType::Qemu => "QEMU",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MountType {
  #[default]
  Virtiofs,
  Sshfs,
  NineP,
}

impl std::fmt::Display for MountType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      MountType::Virtiofs => write!(f, "virtiofs"),
      MountType::Sshfs => write!(f, "sshfs"),
      MountType::NineP => write!(f, "9p"),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColimaVm {
  pub name: String,
  pub status: VmStatus,
  pub runtime: VmRuntime,
  pub arch: VmArch,
  pub cpus: u32,
  pub memory: u64,
  pub disk: u64,
  pub kubernetes: bool,
  pub address: Option<String>,
  // Extended fields from status --json
  pub driver: Option<String>,
  pub vm_type: Option<VmType>,
  pub mount_type: Option<MountType>,
  pub docker_socket: Option<String>,
  pub containerd_socket: Option<String>,
  pub hostname: Option<String>,
  pub rosetta: bool,
  pub ssh_agent: bool,
}

impl Default for ColimaVm {
  fn default() -> Self {
    Self {
      name: "default".to_string(),
      status: VmStatus::Unknown,
      runtime: VmRuntime::Docker,
      arch: VmArch::Aarch64,
      cpus: 2,
      memory: 2 * 1024 * 1024 * 1024,
      disk: 60 * 1024 * 1024 * 1024,
      kubernetes: false,
      address: None,
      driver: None,
      vm_type: None,
      mount_type: None,
      docker_socket: None,
      containerd_socket: None,
      hostname: None,
      rosetta: false,
      ssh_agent: false,
    }
  }
}

impl ColimaVm {
  pub fn memory_gb(&self) -> f64 {
    self.memory as f64 / (1024.0 * 1024.0 * 1024.0)
  }

  pub fn disk_gb(&self) -> f64 {
    self.disk as f64 / (1024.0 * 1024.0 * 1024.0)
  }

  pub fn display_driver(&self) -> String {
    self.driver.clone().unwrap_or_else(|| {
      self
        .vm_type
        .map(|v| v.display_name().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
    })
  }

  pub fn display_mount_type(&self) -> String {
    self
      .mount_type
      .map(|m| m.to_string())
      .unwrap_or_else(|| "virtiofs".to_string())
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColimaStartOptions {
  pub name: Option<String>,
  pub cpus: Option<u32>,
  pub memory: Option<u32>,
  pub disk: Option<u32>,
  pub runtime: Option<VmRuntime>,
  pub kubernetes: bool,
  pub arch: Option<VmArch>,
  pub vm_type: Option<VmType>,
  pub mount_type: Option<MountType>,
  pub network_address: bool,
  pub rosetta: bool,
  pub ssh_agent: bool,
  pub hostname: Option<String>,
  pub edit: bool,
}

/// Information about the VM's operating system
#[derive(Debug, Clone, Default)]
pub struct VmOsInfo {
  pub pretty_name: String,
  pub name: String,
  pub version: String,
  pub version_id: String,
  pub id: String,
  pub kernel: String,
  pub arch: String,
}

/// A file entry in the VM filesystem
#[derive(Debug, Clone)]
pub struct VmFileEntry {
  pub name: String,
  pub path: String,
  pub is_dir: bool,
  pub is_symlink: bool,
  pub size: u64,
  pub permissions: String,
  pub owner: String,
  pub modified: String,
}

impl VmFileEntry {
  pub fn display_size(&self) -> String {
    if self.is_dir {
      "-".to_string()
    } else if self.size < 1024 {
      format!("{} B", self.size)
    } else if self.size < 1024 * 1024 {
      format!("{:.1} KB", self.size as f64 / 1024.0)
    } else if self.size < 1024 * 1024 * 1024 {
      format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
    } else {
      format!("{:.1} GB", self.size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
  }
}

#[allow(dead_code)]
impl ColimaStartOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_name(mut self, name: impl Into<String>) -> Self {
    self.name = Some(name.into());
    self
  }

  pub fn with_cpus(mut self, cpus: u32) -> Self {
    self.cpus = Some(cpus);
    self
  }

  pub fn with_memory_gb(mut self, memory_gb: u32) -> Self {
    self.memory = Some(memory_gb);
    self
  }

  pub fn with_disk_gb(mut self, disk_gb: u32) -> Self {
    self.disk = Some(disk_gb);
    self
  }

  pub fn with_runtime(mut self, runtime: VmRuntime) -> Self {
    self.runtime = Some(runtime);
    self
  }

  pub fn with_kubernetes(mut self, enabled: bool) -> Self {
    self.kubernetes = enabled;
    self
  }

  pub fn with_arch(mut self, arch: VmArch) -> Self {
    self.arch = Some(arch);
    self
  }

  pub fn with_vm_type(mut self, vm_type: VmType) -> Self {
    self.vm_type = Some(vm_type);
    self
  }

  pub fn with_mount_type(mut self, mount_type: MountType) -> Self {
    self.mount_type = Some(mount_type);
    self
  }

  pub fn with_network_address(mut self, enabled: bool) -> Self {
    self.network_address = enabled;
    self
  }

  pub fn with_rosetta(mut self, enabled: bool) -> Self {
    self.rosetta = enabled;
    self
  }

  pub fn with_ssh_agent(mut self, enabled: bool) -> Self {
    self.ssh_agent = enabled;
    self
  }

  pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
    self.hostname = Some(hostname.into());
    self
  }

  pub fn with_edit(mut self, edit: bool) -> Self {
    self.edit = edit;
    self
  }
}
