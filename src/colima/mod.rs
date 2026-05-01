//! Colima VM management module
//!
//! Type definitions are available on all platforms so that `Machine`,
//! `MachineId`, and related enums can be used in cross-platform code.
//!
//! The `ColimaClient` (which shells out to the `colima` CLI) is only
//! functional on macOS and Linux. On Windows a stub is provided that
//! returns errors / empty results so callers compile unchanged.

mod types;

pub use types::*;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod client;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use client::*;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod client_stub {
  use anyhow::{Result, anyhow};

  use super::{ColimaConfig, ColimaVm, VmFileEntry, VmOsInfo};

  fn unsupported<T>() -> Result<T> {
    Err(anyhow!("Colima is not supported on this platform"))
  }

  pub struct ColimaClient;

  impl ColimaClient {
    pub fn new() -> Self {
      Self
    }

    pub fn version() -> Result<String> {
      unsupported()
    }

    pub fn list() -> Result<Vec<ColimaVm>> {
      Ok(Vec::new())
    }

    pub fn status(_name: Option<&str>) -> Result<ColimaVm> {
      unsupported()
    }

    pub fn start_with_config(_profile: &str, _config: &ColimaConfig) -> Result<()> {
      unsupported()
    }

    pub fn start_existing(_name: Option<&str>) -> Result<()> {
      unsupported()
    }

    pub fn stop(_name: Option<&str>) -> Result<()> {
      unsupported()
    }

    pub fn restart(_name: Option<&str>) -> Result<()> {
      unsupported()
    }

    pub fn delete(_name: Option<&str>, _force: bool) -> Result<()> {
      unsupported()
    }

    pub fn socket_path(_name: Option<&str>) -> String {
      String::new()
    }

    pub fn run_command(_name: Option<&str>, _command: &str) -> Result<String> {
      unsupported()
    }

    pub fn get_os_info(_name: Option<&str>) -> Result<VmOsInfo> {
      unsupported()
    }

    pub fn get_system_logs(_name: Option<&str>, _lines: u32) -> Result<String> {
      unsupported()
    }

    pub fn get_docker_logs(_name: Option<&str>, _lines: u32) -> Result<String> {
      unsupported()
    }

    pub fn get_containerd_logs(_name: Option<&str>, _lines: u32) -> Result<String> {
      unsupported()
    }

    pub fn list_files(_name: Option<&str>, _path: &str) -> Result<Vec<VmFileEntry>> {
      Ok(Vec::new())
    }

    pub fn read_file(_name: Option<&str>, _path: &str, _max_lines: u32) -> Result<String> {
      unsupported()
    }

    pub fn resolve_symlink(_name: Option<&str>, _path: &str) -> Result<String> {
      unsupported()
    }

    pub fn is_directory(_name: Option<&str>, _path: &str) -> Result<bool> {
      Ok(false)
    }

    pub fn get_disk_usage(_name: Option<&str>) -> Result<String> {
      unsupported()
    }

    pub fn get_memory_info(_name: Option<&str>) -> Result<String> {
      unsupported()
    }

    pub fn get_processes(_name: Option<&str>) -> Result<String> {
      unsupported()
    }

    pub fn config_path(_name: Option<&str>) -> std::path::PathBuf {
      std::path::PathBuf::new()
    }

    pub fn read_config(_name: Option<&str>) -> Result<ColimaConfig> {
      unsupported()
    }

    pub fn write_config(_name: Option<&str>, _config: &ColimaConfig) -> Result<()> {
      unsupported()
    }

    pub fn update(_name: Option<&str>) -> Result<()> {
      unsupported()
    }

    pub fn prune(_all: bool, _force: bool) -> Result<String> {
      unsupported()
    }

    pub fn ssh_config(_name: Option<&str>) -> Result<String> {
      unsupported()
    }

    pub fn cache_size() -> Result<String> {
      Ok("0 B".to_string())
    }

    pub fn run_provision_script(_name: Option<&str>, _script: &str, _as_root: bool) -> Result<String> {
      unsupported()
    }

    pub fn template_path() -> std::path::PathBuf {
      std::path::PathBuf::new()
    }

    pub fn read_template() -> Result<String> {
      unsupported()
    }

    pub fn write_template(_content: &str) -> Result<()> {
      unsupported()
    }
  }

  impl Default for ColimaClient {
    fn default() -> Self {
      Self::new()
    }
  }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub use client_stub::*;
