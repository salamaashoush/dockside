//! Platform detection and Docker runtime abstraction module
//!
//! This module provides cross-platform support for Dockside, enabling it to work on:
//! - **macOS**: Native Docker runtime via Colima or custom socket paths
//! - **Linux**: Native Docker runtime (`/var/run/docker.sock`) or Colima
//! - **Windows**: Native app connecting to Docker in WSL2 via TCP/HTTP

mod detection;
mod docker_runtime;
mod paths;

#[cfg(target_os = "windows")]
pub mod wsl;

pub use detection::Platform;
pub use docker_runtime::DockerRuntime;
pub use paths::{get_binary_search_paths, get_config_dir, get_home_dir};
