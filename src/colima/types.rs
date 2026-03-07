// Allow precision loss for display formatting of byte sizes
#![allow(clippy::cast_precision_loss)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::docker::DockerHostInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VmStatus {
  Running,
  Stopped,
  #[default]
  Unknown,
}

impl VmStatus {
  pub fn is_running(self) -> bool {
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
#[serde(rename_all = "lowercase")]
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
#[serde(rename_all = "lowercase")]
pub enum VmArch {
  #[default]
  #[serde(alias = "host")]
  Host,
  #[serde(alias = "aarch64")]
  Aarch64,
  #[serde(rename = "x86_64", alias = "x86_64")]
  X86_64,
}

impl VmArch {
  pub fn display_name(self) -> &'static str {
    match self {
      VmArch::Host => "Host (native)",
      VmArch::Aarch64 => "ARM64 (aarch64)",
      VmArch::X86_64 => "x86_64",
    }
  }
}

impl std::fmt::Display for VmArch {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VmArch::Host => write!(f, "host"),
      VmArch::Aarch64 => write!(f, "aarch64"),
      VmArch::X86_64 => write!(f, "x86_64"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VmType {
  #[default]
  Qemu,
  Vz,
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
  pub fn display_name(self) -> &'static str {
    match self {
      VmType::Vz => "Apple Virtualization (VZ)",
      VmType::Qemu => "QEMU",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MountType {
  #[default]
  Sshfs,
  Virtiofs,
  #[serde(rename = "9p")]
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

/// Network mode for the VM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
  #[default]
  Shared,
  Bridged,
}

impl std::fmt::Display for NetworkMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      NetworkMode::Shared => write!(f, "shared"),
      NetworkMode::Bridged => write!(f, "bridged"),
    }
  }
}

/// Port forwarder method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PortForwarder {
  #[default]
  Ssh,
  Grpc,
}

impl std::fmt::Display for PortForwarder {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PortForwarder::Ssh => write!(f, "ssh"),
      PortForwarder::Grpc => write!(f, "grpc"),
    }
  }
}

/// Configuration for a directory mount
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountConfig {
  pub location: String,
  pub writable: bool,
}

impl MountConfig {
  pub fn new(location: impl Into<String>, writable: bool) -> Self {
    Self {
      location: location.into(),
      writable,
    }
  }
}

/// Provision script mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProvisionMode {
  #[default]
  System,
  User,
}

impl std::fmt::Display for ProvisionMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ProvisionMode::System => write!(f, "system"),
      ProvisionMode::User => write!(f, "user"),
    }
  }
}

/// A provisioning script configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionScript {
  pub mode: ProvisionMode,
  pub script: String,
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
        .map_or_else(|| "Unknown".to_string(), |v| v.display_name().to_string())
    })
  }

  pub fn display_mount_type(&self) -> String {
    self
      .mount_type
      .map_or_else(|| "virtiofs".to_string(), |m| m.to_string())
  }
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

// ============================================================================
// Configuration file structures (for reading/writing colima.yaml)
// ============================================================================

/// Kubernetes cluster configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KubernetesConfig {
  pub enabled: bool,
  #[serde(default)]
  pub version: String,
  #[serde(default, rename = "k3sArgs")]
  pub k3s_args: Vec<String>,
  #[serde(default = "default_k8s_port")]
  pub port: u32,
}

fn default_k8s_port() -> u32 {
  6443
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
  #[serde(default)]
  pub address: bool,
  #[serde(default = "default_network_mode")]
  pub mode: NetworkMode,
  #[serde(default = "default_interface", rename = "interface")]
  pub interface: String,
  #[serde(default, rename = "preferredRoute")]
  pub preferred_route: bool,
  #[serde(default)]
  pub dns: Vec<String>,
  #[serde(default = "default_dns_hosts", rename = "dnsHosts")]
  pub dns_hosts: HashMap<String, String>,
  #[serde(default, rename = "hostAddresses")]
  pub host_addresses: bool,
}

fn default_network_mode() -> NetworkMode {
  NetworkMode::Shared
}

fn default_interface() -> String {
  "en0".to_string()
}

fn default_dns_hosts() -> HashMap<String, String> {
  let mut hosts = HashMap::new();
  hosts.insert("host.docker.internal".to_string(), "host.lima.internal".to_string());
  hosts
}

impl Default for NetworkConfig {
  fn default() -> Self {
    Self {
      address: false,
      mode: NetworkMode::Shared,
      interface: "en0".to_string(),
      preferred_route: false,
      dns: Vec::new(),
      dns_hosts: default_dns_hosts(),
      host_addresses: false,
    }
  }
}

/// Full colima configuration (maps to colima.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColimaConfig {
  #[serde(default = "default_cpu")]
  pub cpu: u32,
  #[serde(default = "default_disk")]
  pub disk: u32,
  #[serde(default = "default_memory")]
  pub memory: u32,
  #[serde(default)]
  pub arch: VmArch,
  #[serde(default)]
  pub runtime: VmRuntime,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub hostname: String,
  #[serde(default)]
  pub kubernetes: KubernetesConfig,
  #[serde(default = "default_true", rename = "autoActivate")]
  pub auto_activate: bool,
  #[serde(default)]
  pub network: NetworkConfig,
  #[serde(default, rename = "forwardAgent")]
  pub forward_agent: bool,
  #[serde(default, skip_serializing_if = "is_empty_object")]
  pub docker: serde_json::Value,
  #[serde(default, rename = "vmType")]
  pub vm_type: VmType,
  #[serde(default, rename = "portForwarder")]
  pub port_forwarder: PortForwarder,
  #[serde(default)]
  pub rosetta: bool,
  #[serde(default = "default_true")]
  pub binfmt: bool,
  #[serde(default, rename = "nestedVirtualization")]
  pub nested_virtualization: bool,
  #[serde(default, rename = "mountType")]
  pub mount_type: MountType,
  #[serde(default, rename = "mountInotify")]
  pub mount_inotify: bool,
  #[serde(default = "default_cpu_type", rename = "cpuType")]
  pub cpu_type: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub provision: Vec<ProvisionScript>,
  #[serde(default = "default_true", rename = "sshConfig")]
  pub ssh_config: bool,
  #[serde(default, rename = "sshPort")]
  pub ssh_port: u32,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub mounts: Vec<MountConfig>,
  #[serde(default, skip_serializing_if = "String::is_empty", rename = "diskImage")]
  pub disk_image: String,
  #[serde(default = "default_root_disk", rename = "rootDisk")]
  pub root_disk: u32,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  pub env: HashMap<String, String>,
}

fn is_empty_object(v: &serde_json::Value) -> bool {
  v.as_object().is_some_and(serde_json::Map::is_empty)
}

fn default_cpu() -> u32 {
  2
}

fn default_memory() -> u32 {
  2
}

fn default_disk() -> u32 {
  100
}

fn default_root_disk() -> u32 {
  20
}

fn default_cpu_type() -> String {
  "host".to_string()
}

fn default_true() -> bool {
  true
}

impl Default for ColimaConfig {
  fn default() -> Self {
    Self {
      cpu: default_cpu(),
      disk: default_disk(),
      memory: default_memory(),
      arch: VmArch::default(),
      runtime: VmRuntime::default(),
      hostname: String::new(),
      kubernetes: KubernetesConfig::default(),
      auto_activate: true,
      network: NetworkConfig::default(),
      forward_agent: false,
      docker: serde_json::Value::Object(serde_json::Map::new()),
      vm_type: VmType::default(),
      port_forwarder: PortForwarder::default(),
      rosetta: false,
      binfmt: true,
      nested_virtualization: false,
      mount_type: MountType::default(),
      mount_inotify: false,
      cpu_type: default_cpu_type(),
      provision: Vec::new(),
      ssh_config: true,
      ssh_port: 0,
      mounts: Vec::new(),
      disk_image: String::new(),
      root_disk: default_root_disk(),
      env: HashMap::new(),
    }
  }
}

// ============================================================================
// Machine types (for unified Host + Colima VM representation)
// ============================================================================

/// Unique identifier for a machine (Host or Colima VM)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MachineId {
  /// The native Docker host
  Host,
  /// A Colima VM by name
  Colima(String),
}

impl MachineId {
  /// Get a display name for this machine ID
  pub fn name(&self) -> &str {
    match self {
      MachineId::Host => "Host",
      MachineId::Colima(name) => name,
    }
  }

  /// Check if this is the Host machine
  pub fn is_host(&self) -> bool {
    matches!(self, MachineId::Host)
  }

  /// Check if this is a Colima VM
  pub fn is_colima(&self) -> bool {
    matches!(self, MachineId::Colima(_))
  }
}

impl std::fmt::Display for MachineId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      MachineId::Host => write!(f, "Host"),
      MachineId::Colima(name) => write!(f, "{name}"),
    }
  }
}

/// A machine that can run Docker - either the native host or a Colima VM
#[derive(Debug, Clone)]
pub enum Machine {
  /// Native Docker host (Linux with Docker daemon)
  Host(DockerHostInfo),
  /// Colima virtual machine
  Colima(ColimaVm),
}

impl Machine {
  /// Get the unique identifier for this machine
  pub fn id(&self) -> MachineId {
    match self {
      Machine::Host(_) => MachineId::Host,
      Machine::Colima(vm) => MachineId::Colima(vm.name.clone()),
    }
  }

  /// Get the display name of this machine
  pub fn name(&self) -> &str {
    match self {
      Machine::Host(h) => &h.name,
      Machine::Colima(vm) => &vm.name,
    }
  }

  /// Check if this machine is currently running
  pub fn is_running(&self) -> bool {
    match self {
      // Host is running if Docker is connected
      Machine::Host(_) => true,
      Machine::Colima(vm) => vm.status.is_running(),
    }
  }

  /// Check if this is the Host machine
  pub fn is_host(&self) -> bool {
    matches!(self, Machine::Host(_))
  }

  /// Check if this is a Colima VM
  pub fn is_colima(&self) -> bool {
    matches!(self, Machine::Colima(_))
  }

  /// Get the Docker socket path for this machine
  pub fn docker_socket(&self) -> Option<String> {
    match self {
      Machine::Host(h) => Some(h.docker_socket.clone()),
      Machine::Colima(vm) => vm.docker_socket.clone(),
    }
  }

  /// Get the number of CPUs
  pub fn cpus(&self) -> u32 {
    match self {
      Machine::Host(h) => h.cpus,
      Machine::Colima(vm) => vm.cpus,
    }
  }

  /// Get the memory in bytes
  pub fn memory(&self) -> u64 {
    match self {
      Machine::Host(h) => h.memory,
      Machine::Colima(vm) => vm.memory,
    }
  }

  /// Get the architecture as a string
  pub fn arch(&self) -> &str {
    match self {
      Machine::Host(h) => &h.arch,
      Machine::Colima(vm) => match vm.arch {
        VmArch::Host => "host",
        VmArch::Aarch64 => "aarch64",
        VmArch::X86_64 => "x86_64",
      },
    }
  }

  /// Get the operating system name
  pub fn os(&self) -> &str {
    match self {
      Machine::Host(h) => &h.os,
      Machine::Colima(_) => "Linux (VM)",
    }
  }

  /// Get the Docker version if available
  pub fn docker_version(&self) -> Option<&str> {
    match self {
      Machine::Host(h) => Some(&h.docker_version),
      Machine::Colima(_) => None,
    }
  }

  /// Get memory in gigabytes for display
  pub fn memory_gb(&self) -> f64 {
    self.memory() as f64 / (1024.0 * 1024.0 * 1024.0)
  }

  /// Get formatted memory string
  pub fn display_memory(&self) -> String {
    let gb = self.memory_gb();
    if gb >= 1.0 {
      format!("{:.1} GB", gb)
    } else {
      let mb = self.memory() as f64 / (1024.0 * 1024.0);
      format!("{:.0} MB", mb)
    }
  }

  /// Get status display string
  pub fn status_display(&self) -> &'static str {
    match self {
      Machine::Host(_) => "Running",
      Machine::Colima(vm) => match vm.status {
        VmStatus::Running => "Running",
        VmStatus::Stopped => "Stopped",
        VmStatus::Unknown => "Unknown",
      },
    }
  }

  /// Get the machine type as a string
  pub fn machine_type(&self) -> &'static str {
    match self {
      Machine::Host(_) => "Native Docker",
      Machine::Colima(_) => "Colima VM",
    }
  }

  /// Get the underlying ColimaVm if this is a Colima machine
  pub fn as_colima(&self) -> Option<&ColimaVm> {
    match self {
      Machine::Colima(vm) => Some(vm),
      Machine::Host(_) => None,
    }
  }

  /// Get the underlying DockerHostInfo if this is a Host machine
  pub fn as_host(&self) -> Option<&DockerHostInfo> {
    match self {
      Machine::Host(h) => Some(h),
      Machine::Colima(_) => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_vm_status_is_running() {
    assert!(VmStatus::Running.is_running());
    assert!(!VmStatus::Stopped.is_running());
    assert!(!VmStatus::Unknown.is_running());
  }

  #[test]
  fn test_vm_status_display() {
    assert_eq!(format!("{}", VmStatus::Running), "Running");
    assert_eq!(format!("{}", VmStatus::Stopped), "Stopped");
    assert_eq!(format!("{}", VmStatus::Unknown), "Unknown");
  }

  #[test]
  fn test_vm_status_default() {
    assert_eq!(VmStatus::default(), VmStatus::Unknown);
  }

  #[test]
  fn test_vm_runtime_display() {
    assert_eq!(format!("{}", VmRuntime::Docker), "docker");
    assert_eq!(format!("{}", VmRuntime::Containerd), "containerd");
    assert_eq!(format!("{}", VmRuntime::Incus), "incus");
  }

  #[test]
  fn test_vm_runtime_default() {
    assert_eq!(VmRuntime::default(), VmRuntime::Docker);
  }

  #[test]
  fn test_vm_arch_display_name() {
    assert_eq!(VmArch::Host.display_name(), "Host (native)");
    assert_eq!(VmArch::Aarch64.display_name(), "ARM64 (aarch64)");
    assert_eq!(VmArch::X86_64.display_name(), "x86_64");
  }

  #[test]
  fn test_vm_arch_display() {
    assert_eq!(format!("{}", VmArch::Host), "host");
    assert_eq!(format!("{}", VmArch::Aarch64), "aarch64");
    assert_eq!(format!("{}", VmArch::X86_64), "x86_64");
  }

  #[test]
  fn test_vm_type_display_name() {
    assert_eq!(VmType::Vz.display_name(), "Apple Virtualization (VZ)");
    assert_eq!(VmType::Qemu.display_name(), "QEMU");
  }

  #[test]
  fn test_vm_type_display() {
    assert_eq!(format!("{}", VmType::Vz), "vz");
    assert_eq!(format!("{}", VmType::Qemu), "qemu");
  }

  #[test]
  fn test_vm_type_default() {
    assert_eq!(VmType::default(), VmType::Qemu);
  }

  #[test]
  fn test_mount_type_display() {
    assert_eq!(format!("{}", MountType::Virtiofs), "virtiofs");
    assert_eq!(format!("{}", MountType::Sshfs), "sshfs");
    assert_eq!(format!("{}", MountType::NineP), "9p");
  }

  #[test]
  fn test_mount_type_default() {
    assert_eq!(MountType::default(), MountType::Sshfs);
  }

  #[test]
  fn test_network_mode_display() {
    assert_eq!(format!("{}", NetworkMode::Shared), "shared");
    assert_eq!(format!("{}", NetworkMode::Bridged), "bridged");
  }

  #[test]
  fn test_network_mode_default() {
    assert_eq!(NetworkMode::default(), NetworkMode::Shared);
  }

  #[test]
  fn test_port_forwarder_display() {
    assert_eq!(format!("{}", PortForwarder::Ssh), "ssh");
    assert_eq!(format!("{}", PortForwarder::Grpc), "grpc");
  }

  #[test]
  fn test_port_forwarder_default() {
    assert_eq!(PortForwarder::default(), PortForwarder::Ssh);
  }

  #[test]
  fn test_mount_config_new() {
    let mount = MountConfig::new("/home/user", true);
    assert_eq!(mount.location, "/home/user");
    assert!(mount.writable);

    let readonly = MountConfig::new(String::from("/data"), false);
    assert_eq!(readonly.location, "/data");
    assert!(!readonly.writable);
  }

  #[test]
  fn test_provision_mode_display() {
    assert_eq!(format!("{}", ProvisionMode::System), "system");
    assert_eq!(format!("{}", ProvisionMode::User), "user");
  }

  #[test]
  fn test_provision_mode_default() {
    assert_eq!(ProvisionMode::default(), ProvisionMode::System);
  }

  #[test]
  fn test_colima_vm_default() {
    let vm = ColimaVm::default();
    assert_eq!(vm.name, "default");
    assert_eq!(vm.status, VmStatus::Unknown);
    assert_eq!(vm.runtime, VmRuntime::Docker);
    assert_eq!(vm.arch, VmArch::Aarch64);
    assert_eq!(vm.cpus, 2);
    assert_eq!(vm.memory, 2 * 1024 * 1024 * 1024);
    assert_eq!(vm.disk, 60 * 1024 * 1024 * 1024);
    assert!(!vm.kubernetes);
    assert!(vm.address.is_none());
  }

  #[test]
  fn test_colima_vm_memory_gb() {
    let vm = ColimaVm {
      memory: 4 * 1024 * 1024 * 1024, // 4 GB
      ..Default::default()
    };
    assert!((vm.memory_gb() - 4.0).abs() < 0.01);
  }

  #[test]
  fn test_colima_vm_disk_gb() {
    let vm = ColimaVm {
      disk: 100 * 1024 * 1024 * 1024, // 100 GB
      ..Default::default()
    };
    assert!((vm.disk_gb() - 100.0).abs() < 0.01);
  }

  #[test]
  fn test_colima_vm_display_driver() {
    // With driver set
    let vm_with_driver = ColimaVm {
      driver: Some("custom-driver".to_string()),
      ..Default::default()
    };
    assert_eq!(vm_with_driver.display_driver(), "custom-driver");

    // Without driver, with vm_type
    let vm_with_type = ColimaVm {
      driver: None,
      vm_type: Some(VmType::Vz),
      ..Default::default()
    };
    assert_eq!(vm_with_type.display_driver(), "Apple Virtualization (VZ)");

    // Without driver or vm_type
    let vm_unknown = ColimaVm {
      driver: None,
      vm_type: None,
      ..Default::default()
    };
    assert_eq!(vm_unknown.display_driver(), "Unknown");
  }

  #[test]
  fn test_colima_vm_display_mount_type() {
    let vm_with_mount = ColimaVm {
      mount_type: Some(MountType::Virtiofs),
      ..Default::default()
    };
    assert_eq!(vm_with_mount.display_mount_type(), "virtiofs");

    let vm_without_mount = ColimaVm {
      mount_type: None,
      ..Default::default()
    };
    assert_eq!(vm_without_mount.display_mount_type(), "virtiofs");
  }

  #[test]
  fn test_vm_file_entry_display_size() {
    // Directory
    let dir = VmFileEntry {
      name: "test".to_string(),
      path: "/test".to_string(),
      is_dir: true,
      is_symlink: false,
      size: 4096,
      permissions: "drwxr-xr-x".to_string(),
      owner: "root".to_string(),
      modified: "2024-01-01".to_string(),
    };
    assert_eq!(dir.display_size(), "-");

    // Small file (bytes)
    let small = VmFileEntry {
      is_dir: false,
      size: 500,
      ..dir.clone()
    };
    assert_eq!(small.display_size(), "500 B");

    // KB file
    let kb = VmFileEntry {
      is_dir: false,
      size: 1536, // 1.5 KB
      ..dir.clone()
    };
    assert_eq!(kb.display_size(), "1.5 KB");

    // MB file
    let mb = VmFileEntry {
      is_dir: false,
      size: 5 * 1024 * 1024, // 5 MB
      ..dir.clone()
    };
    assert_eq!(mb.display_size(), "5.0 MB");

    // GB file
    let gb = VmFileEntry {
      is_dir: false,
      size: 2 * 1024 * 1024 * 1024, // 2 GB
      ..dir.clone()
    };
    assert_eq!(gb.display_size(), "2.0 GB");
  }

  #[test]
  fn test_kubernetes_config_default() {
    let k8s = KubernetesConfig::default();
    assert!(!k8s.enabled);
    assert!(k8s.version.is_empty());
    assert!(k8s.k3s_args.is_empty());
    // Port defaults to 0 in struct, serde uses default_k8s_port (6443) during deserialization
    assert_eq!(k8s.port, 0);
  }

  #[test]
  fn test_network_config_default() {
    let net = NetworkConfig::default();
    assert!(!net.address);
    assert_eq!(net.mode, NetworkMode::Shared);
    assert_eq!(net.interface, "en0");
    assert!(!net.preferred_route);
    assert!(net.dns.is_empty());
    assert!(net.dns_hosts.contains_key("host.docker.internal"));
    assert!(!net.host_addresses);
  }

  #[test]
  fn test_colima_config_default() {
    let config = ColimaConfig::default();
    assert_eq!(config.cpu, 2);
    assert_eq!(config.memory, 2);
    assert_eq!(config.disk, 100);
    assert_eq!(config.arch, VmArch::Host);
    assert_eq!(config.runtime, VmRuntime::Docker);
    assert!(config.hostname.is_empty());
    assert!(config.auto_activate);
    assert!(!config.forward_agent);
    assert_eq!(config.vm_type, VmType::Qemu);
    assert_eq!(config.port_forwarder, PortForwarder::Ssh);
    assert!(!config.rosetta);
    assert!(config.binfmt);
    assert!(!config.nested_virtualization);
    assert_eq!(config.mount_type, MountType::Sshfs);
    assert!(!config.mount_inotify);
    assert_eq!(config.cpu_type, "host");
    assert!(config.provision.is_empty());
    assert!(config.ssh_config);
    assert_eq!(config.ssh_port, 0);
    assert!(config.mounts.is_empty());
    assert!(config.disk_image.is_empty());
    assert_eq!(config.root_disk, 20);
    assert!(config.env.is_empty());
  }

  #[test]
  fn test_colima_config_serialization() {
    let config = ColimaConfig::default();
    let yaml = serde_yaml::to_string(&config).expect("Failed to serialize");
    assert!(yaml.contains("cpu: 2"));
    assert!(yaml.contains("memory: 2"));
    assert!(yaml.contains("disk: 100"));
  }

  // MachineId tests
  #[test]
  fn test_machine_id_host() {
    let id = MachineId::Host;
    assert!(id.is_host());
    assert!(!id.is_colima());
    assert_eq!(id.name(), "Host");
    assert_eq!(format!("{}", id), "Host");
  }

  #[test]
  fn test_machine_id_colima() {
    let id = MachineId::Colima("dev".to_string());
    assert!(!id.is_host());
    assert!(id.is_colima());
    assert_eq!(id.name(), "dev");
    assert_eq!(format!("{}", id), "dev");
  }

  #[test]
  fn test_machine_id_equality() {
    let host1 = MachineId::Host;
    let host2 = MachineId::Host;
    let colima1 = MachineId::Colima("default".to_string());
    let colima2 = MachineId::Colima("default".to_string());
    let colima3 = MachineId::Colima("other".to_string());

    assert_eq!(host1, host2);
    assert_eq!(colima1, colima2);
    assert_ne!(host1, colima1);
    assert_ne!(colima1, colima3);
  }

  // Machine tests
  fn create_test_host_info() -> DockerHostInfo {
    DockerHostInfo {
      name: "test-host".to_string(),
      os: "Arch Linux".to_string(),
      kernel: "6.18.6-arch1-1".to_string(),
      arch: "x86_64".to_string(),
      cpus: 16,
      memory: 32 * 1024 * 1024 * 1024,
      docker_version: "27.0.3".to_string(),
      api_version: "1.46".to_string(),
      storage_driver: "overlay2".to_string(),
      docker_root: "/var/lib/docker".to_string(),
      containers_total: 12,
      containers_running: 5,
      containers_paused: 0,
      containers_stopped: 7,
      images: 47,
      runtime: "runc".to_string(),
      docker_socket: "/var/run/docker.sock".to_string(),
    }
  }

  #[test]
  fn test_machine_host() {
    let host_info = create_test_host_info();
    let machine = Machine::Host(host_info);

    assert!(machine.is_host());
    assert!(!machine.is_colima());
    assert!(machine.is_running());
    assert_eq!(machine.name(), "test-host");
    assert_eq!(machine.id(), MachineId::Host);
    assert_eq!(machine.cpus(), 16);
    assert_eq!(machine.arch(), "x86_64");
    assert_eq!(machine.os(), "Arch Linux");
    assert_eq!(machine.docker_version(), Some("27.0.3"));
    assert_eq!(machine.status_display(), "Running");
    assert_eq!(machine.machine_type(), "Native Docker");
    assert!(machine.as_host().is_some());
    assert!(machine.as_colima().is_none());
  }

  #[test]
  fn test_machine_colima() {
    let vm = ColimaVm {
      name: "dev".to_string(),
      status: VmStatus::Running,
      cpus: 4,
      memory: 8 * 1024 * 1024 * 1024,
      arch: VmArch::Aarch64,
      docker_socket: Some("/Users/test/.colima/dev/docker.sock".to_string()),
      ..Default::default()
    };
    let machine = Machine::Colima(vm);

    assert!(!machine.is_host());
    assert!(machine.is_colima());
    assert!(machine.is_running());
    assert_eq!(machine.name(), "dev");
    assert_eq!(machine.id(), MachineId::Colima("dev".to_string()));
    assert_eq!(machine.cpus(), 4);
    assert_eq!(machine.arch(), "aarch64");
    assert_eq!(machine.os(), "Linux (VM)");
    assert_eq!(machine.docker_version(), None);
    assert_eq!(machine.status_display(), "Running");
    assert_eq!(machine.machine_type(), "Colima VM");
    assert!(machine.as_colima().is_some());
    assert!(machine.as_host().is_none());
  }

  #[test]
  fn test_machine_colima_stopped() {
    let vm = ColimaVm {
      name: "stopped-vm".to_string(),
      status: VmStatus::Stopped,
      ..Default::default()
    };
    let machine = Machine::Colima(vm);

    assert!(!machine.is_running());
    assert_eq!(machine.status_display(), "Stopped");
  }

  #[test]
  fn test_machine_memory_display() {
    let host_info = create_test_host_info();
    let machine = Machine::Host(host_info);
    assert!((machine.memory_gb() - 32.0).abs() < 0.01);
    assert_eq!(machine.display_memory(), "32.0 GB");
  }
}
