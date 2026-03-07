//! Host Docker daemon operations
//!
//! Operations specific to the native Docker host (Linux systems with Docker daemon).
//! These operations are not applicable to Colima VMs.

use gpui::App;

use crate::docker::DaemonConfig;
use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{StateChanged, docker_state};

use super::core::{DispatcherEvent, dispatcher};

/// Restart the Docker daemon on the host system
///
/// This requires appropriate permissions (typically root/sudo).
/// On systemd-based systems, this uses `systemctl restart docker`.
pub fn restart_docker_daemon(cx: &mut App) {
  let task_id = start_task(cx, "Restarting Docker daemon...".to_string());
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = tokio::process::Command::new("sudo")
      .args(["systemctl", "restart", "docker"])
      .output()
      .await;

    let _ = cx.update(|cx| match result {
      Ok(output) if output.status.success() => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Docker daemon restarted".to_string(),
          });
        });
      }
      Ok(output) => {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error = if stderr.is_empty() {
          "Failed to restart Docker daemon".to_string()
        } else {
          format!("Failed to restart Docker: {stderr}")
        };
        fail_task(cx, task_id, error.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error });
        });
      }
      Err(e) => {
        let error = format!("Failed to restart Docker: {e}");
        fail_task(cx, task_id, error.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error });
        });
      }
    });
  })
  .detach();
}

/// Run docker system prune on the host
///
/// Removes unused containers, networks, images, and optionally volumes.
pub fn docker_system_prune(prune_volumes: bool, cx: &mut App) {
  let task_id = start_task(cx, "Pruning Docker system...".to_string());
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    use crate::services::core::docker_client;

    let client_handle = docker_client();
    let guard = client_handle.read().await;

    if let Some(docker) = guard.as_ref() {
      // Use individual prune operations
      let _ = docker.prune_containers().await;
      let _ = docker.prune_networks().await;
      let _ = docker.prune_images(false).await;
      if prune_volumes {
        let _ = docker.prune_volumes().await;
      }
      Ok::<_, String>(())
    } else {
      Err("Docker client not connected".to_string())
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(_)) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Docker system pruned".to_string(),
          });
        });
      }
      Ok(Err(e)) => {
        let error = format!("Prune failed: {e}");
        fail_task(cx, task_id, error.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error });
        });
      }
      Err(e) => {
        let error = format!("Prune failed: {e}");
        fail_task(cx, task_id, error.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error });
        });
      }
    })
  })
  .detach();
}

/// Read the Docker daemon.json configuration file
pub async fn read_daemon_config() -> anyhow::Result<DaemonConfig> {
  DaemonConfig::load_default().await
}

/// Write the Docker daemon.json configuration file
///
/// Note: Writing to /etc/docker/daemon.json typically requires root permissions.
pub async fn write_daemon_config(config: &DaemonConfig) -> anyhow::Result<()> {
  config.save_default().await
}

/// Get the default daemon.json path for the current platform
pub fn daemon_json_path() -> std::path::PathBuf {
  DaemonConfig::default_path()
}

/// Refresh host Docker system info
pub fn refresh_host_info(cx: &mut App) {
  let state = docker_state(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    use crate::services::core::docker_client;

    let client_handle = docker_client();
    let guard = client_handle.read().await;

    if let Some(docker) = guard.as_ref() {
      docker.get_system_info().await.ok()
    } else {
      None
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      if let Ok(Some(host_info)) = result {
        state.update(cx, |state, cx| {
          // Update the host machine if it exists
          if let Some(host_machine) = state.machines.iter_mut().find(|m| m.is_host()) {
            if let crate::colima::Machine::Host(existing) = host_machine {
              *existing = host_info;
            }
          }
          cx.emit(StateChanged::MachinesUpdated);
        });
      }
    })
  })
  .detach();
}
