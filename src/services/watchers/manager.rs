//! Watcher manager
//!
//! Coordinates Docker, Kubernetes, and Colima watchers and emits debounced
//! refresh signals to the GPUI state layer.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, Global};
use tokio::sync::RwLock;

use super::WatcherControl;
use super::debouncer::{EventDebouncer, ResourceType};
use super::docker_events::DockerEventWatcher;
use super::kubernetes::KubernetesWatcher;
use super::machines::MachineWatcher;
use crate::docker::DockerClient;
use crate::state::settings_state;

/// Manages all resource watchers
pub struct WatcherManager {
  docker_client: Arc<RwLock<Option<DockerClient>>>,
  control: WatcherControl,
  colima_enabled: bool,
}

impl WatcherManager {
  pub fn new(docker_client: Arc<RwLock<Option<DockerClient>>>, colima_enabled: bool) -> Self {
    Self {
      docker_client,
      control: WatcherControl::new(),
      colima_enabled,
    }
  }

  /// Start all watchers
  ///
  /// Spawns background tasks that watch for resource changes and emit
  /// debounced refresh signals to update the UI.
  pub fn start(&self, cx: &mut App) {
    let control = self.control.clone();
    let docker_client = self.docker_client.clone();

    // Create debounced channel (250ms debounce window)
    // All watchers share the same sender
    let (debounce_tx, mut debounce_rx) = EventDebouncer::channel(250);

    // Spawn the refresh handler on GPUI
    cx.spawn(async move |cx| {
      while let Some(resources) = debounce_rx.recv().await {
        let _ = cx.update(|cx| {
          refresh_resources(&resources, cx);
        });
      }
    })
    .detach();

    // Spawn Docker events watcher
    let docker_tx = debounce_tx.clone();
    let docker_control = control.clone();
    crate::services::Tokio::spawn(cx, async move {
      let watcher = DockerEventWatcher::new(docker_client);

      watcher
        .watch(docker_control, |event| {
          tracing::debug!("Docker event: {event:?}");
          docker_tx.send(event.resource_type());
        })
        .await;

      Ok::<(), anyhow::Error>(())
    })
    .detach();

    // Spawn Kubernetes watcher
    let k8s_tx = debounce_tx.clone();
    let k8s_control = control.clone();
    crate::services::Tokio::spawn(cx, async move {
      let watcher = KubernetesWatcher::new().await;

      if watcher.is_available() {
        watcher
          .watch_all(k8s_control, |resource_type| {
            tracing::debug!("Kubernetes change: {resource_type:?}");
            k8s_tx.send(resource_type);
          })
          .await;
      }

      Ok::<(), anyhow::Error>(())
    })
    .detach();

    // Spawn Colima machine watcher (polls every 10 seconds - conservative to avoid overhead)
    // Only if Colima is enabled in settings
    if self.colima_enabled {
      let machine_tx = debounce_tx;
      let machine_control = control;
      crate::services::Tokio::spawn(cx, async move {
        let watcher = MachineWatcher::new(Duration::from_secs(10));

        watcher
          .watch(machine_control, |resource_type| {
            tracing::debug!("Machine change detected");
            machine_tx.send(resource_type);
          })
          .await;

        Ok::<(), anyhow::Error>(())
      })
      .detach();
    }
  }

  /// Stop all watchers gracefully
  pub fn stop(&self) {
    self.control.stop();
  }
}

/// Refresh resources based on the types that changed
fn refresh_resources(resources: &HashSet<ResourceType>, cx: &mut App) {
  for resource in resources {
    match resource {
      ResourceType::Container => {
        crate::services::refresh_containers(cx);
      }
      ResourceType::Image => {
        crate::services::refresh_images(cx);
      }
      ResourceType::Volume => {
        crate::services::refresh_volumes(cx);
      }
      ResourceType::Network => {
        crate::services::refresh_networks(cx);
      }
      ResourceType::Pod => {
        crate::services::refresh_pods(cx);
      }
      ResourceType::Deployment => {
        crate::services::refresh_deployments(cx);
      }
      ResourceType::Service => {
        crate::services::refresh_services(cx);
      }
      ResourceType::Machine => {
        crate::services::refresh_machines(cx);
      }
    }
  }
}

/// Global wrapper for `WatcherManager`
struct GlobalWatcherManager(WatcherManager);

impl Global for GlobalWatcherManager {}

/// Start watchers and store globally
pub fn start_watchers(docker_client: Arc<RwLock<Option<DockerClient>>>, cx: &mut App) {
  let colima_enabled = settings_state(cx).read(cx).settings.colima_enabled;
  let manager = WatcherManager::new(docker_client, colima_enabled);
  manager.start(cx);
  cx.set_global(GlobalWatcherManager(manager));
}

/// Stop all watchers gracefully (call on app shutdown)
pub fn stop_watchers(cx: &App) {
  if let Some(global) = cx.try_global::<GlobalWatcherManager>() {
    global.0.stop();
  }
}
