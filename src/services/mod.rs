//! Application services layer
//!
//! This module contains all the async operations and dispatchers for the application.
//! It is organized into submodules by resource type:
//!
//! - `core` - Dispatcher types and Docker client management
//! - `docker` - Docker resource operations (containers, images, volumes, networks, compose)
//! - `colima` - Colima machine and Kubernetes control operations
//! - `kubernetes` - Kubernetes resource operations (pods, services, deployments)
//! - `navigation` - View and tab navigation functions
//! - `prune` - Docker prune operations
//! - `init` - Initial data loading
//! - `watchers` - Real-time resource watchers for Docker and Kubernetes

mod colima;
mod core;
mod docker;
mod gpui_tokio;
mod host;
mod init;
mod kubernetes;
mod navigation;
mod prune;
mod task_manager;
mod watchers;

// Re-export everything for backward compatibility
pub use colima::*;
pub use core::*;
pub use docker::*;
pub use gpui_tokio::Tokio;
pub use host::*;
pub use init::*;
pub use kubernetes::*;
pub use navigation::*;
pub use prune::*;
pub use task_manager::*;
pub use watchers::stop_watchers;

use gpui::App;

use crate::state::{init_docker_state, init_settings};

/// Initialize all global services
pub fn init_services(cx: &mut App) {
  // Initialize tokio runtime first (required for Docker client)
  gpui_tokio::init(cx);

  // Initialize state
  init_docker_state(cx);
  init_settings(cx);

  // Initialize services
  init_task_manager(cx);
  init_dispatcher(cx);
}

/// Start the real-time resource watchers
///
/// Call this after initial data loading to enable real-time updates.
/// The watchers monitor Docker events and Kubernetes resource changes,
/// automatically refreshing the UI when resources are added, modified, or deleted.
pub fn start_watchers(cx: &mut App) {
  let docker_client = docker_client();
  watchers::start_watchers(docker_client, cx);
}
