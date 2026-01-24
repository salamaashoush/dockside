//! Colima VM management module
//!
//! This module is only available on macOS and Linux platforms.
//! On Windows, stub implementations are provided for type compatibility.

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod client;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod types;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use client::*;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use types::*;

// Stub implementations for Windows
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod stubs {
    use serde::{Deserialize, Serialize};

    /// Colima client stub for Windows
    pub struct ColimaClient;

    impl ColimaClient {
        pub fn new() -> Self {
            Self
        }

        pub fn list() -> anyhow::Result<Vec<ColimaVm>> {
            Ok(Vec::new())
        }

        pub fn socket_path(_name: Option<&str>) -> String {
            String::new()
        }
    }

    impl Default for ColimaClient {
        fn default() -> Self {
            Self::new()
        }
    }

    /// VM status stub
    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub enum VmStatus {
        #[default]
        Stopped,
        Running,
        Unknown,
    }

    impl VmStatus {
        pub fn is_running(&self) -> bool {
            matches!(self, Self::Running)
        }
    }

    /// VM runtime stub
    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub enum VmRuntime {
        #[default]
        Docker,
        Containerd,
        Incus,
    }

    /// VM architecture stub
    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub enum VmArch {
        Host,
        #[default]
        Aarch64,
        X86_64,
    }

    /// VM type stub
    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub enum VmType {
        #[default]
        Qemu,
        Vz,
    }

    /// Mount type stub
    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub enum MountType {
        #[default]
        Sshfs,
        NineP,
        Virtiofs,
    }

    /// Colima VM stub
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
        pub driver: Option<String>,
        pub vm_type: Option<VmType>,
        pub mount_type: Option<MountType>,
        pub docker_socket: Option<String>,
        pub containerd_socket: Option<String>,
        pub hostname: Option<String>,
        pub rosetta: bool,
        pub ssh_agent: bool,
    }

    /// Colima config stub
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ColimaConfig {
        pub cpu: u32,
        pub memory: u64,
        pub disk: u64,
    }

    /// VM file entry stub
    #[derive(Debug, Clone, Default)]
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

    /// VM OS info stub
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
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub use stubs::*;
