use gpui::{App, AppContext, Entity, EventEmitter, Global};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::colima::{ColimaClient, ColimaStartOptions};
use crate::docker::{ContainerCreateConfig, ContainerFlags, DockerClient};
use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{CurrentView, ImageInspectData, StateChanged, docker_state, settings_state};

/// Shared Docker client - initialized once in `load_initial_data`
static DOCKER_CLIENT: std::sync::OnceLock<Arc<RwLock<Option<DockerClient>>>> = std::sync::OnceLock::new();

/// Get the shared Docker client handle
pub fn docker_client() -> Arc<RwLock<Option<DockerClient>>> {
  DOCKER_CLIENT.get_or_init(|| Arc::new(RwLock::new(None))).clone()
}

/// Event emitted when a task completes (for UI to show notifications)
#[derive(Clone, Debug)]
pub enum DispatcherEvent {
  TaskCompleted { message: String },
  TaskFailed { error: String },
}

/// Central action dispatcher - handles all async operations
pub struct ActionDispatcher;

impl ActionDispatcher {
  pub fn new() -> Self {
    Self
  }
}

impl Default for ActionDispatcher {
  fn default() -> Self {
    Self::new()
  }
}

impl EventEmitter<DispatcherEvent> for ActionDispatcher {}

/// Global wrapper
pub struct GlobalActionDispatcher(pub Entity<ActionDispatcher>);

impl Global for GlobalActionDispatcher {}

/// Initialize the global action dispatcher
pub fn init_dispatcher(cx: &mut App) -> Entity<ActionDispatcher> {
  let dispatcher = cx.new(|_cx| ActionDispatcher::new());
  cx.set_global(GlobalActionDispatcher(dispatcher.clone()));
  dispatcher
}

/// Get the global dispatcher
pub fn dispatcher(cx: &App) -> Entity<ActionDispatcher> {
  cx.global::<GlobalActionDispatcher>().0.clone()
}

// ==================== Action Handlers ====================
// These are standalone functions that can be called from anywhere

pub fn create_machine(options: ColimaStartOptions, cx: &mut App) {
  let machine_name = options.name.clone().unwrap_or_else(|| "default".to_string());
  let task_id = start_task(cx, format!("Creating '{machine_name}'..."));
  let name_clone = machine_name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        match ColimaClient::start(&options) {
          Ok(()) => Ok(ColimaClient::list().unwrap_or_default()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(vms) => {
        state.update(cx, |state, cx| {
          state.set_machines(vms);
          cx.emit(StateChanged::MachinesUpdated);
        });
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Machine '{name_clone}' created"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to create '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn start_machine(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Starting '{name}'..."));
  let name_clone = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let options = ColimaStartOptions::new().with_name(name.clone());
        match ColimaClient::start(&options) {
          Ok(()) => Ok(ColimaClient::list().unwrap_or_default()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(vms) => {
        state.update(cx, |state, cx| {
          state.set_machines(vms);
          cx.emit(StateChanged::MachinesUpdated);
        });
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Machine '{name_clone}' started"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to start '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn stop_machine(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Stopping '{name}'..."));
  let name_clone = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };
        match ColimaClient::stop(name_opt) {
          Ok(()) => Ok(ColimaClient::list().unwrap_or_default()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(vms) => {
        state.update(cx, |state, cx| {
          state.set_machines(vms);
          cx.emit(StateChanged::MachinesUpdated);
        });
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Machine '{name_clone}' stopped"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to stop '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn edit_machine(options: ColimaStartOptions, cx: &mut App) {
  let name = options.name.clone().unwrap_or_else(|| "default".to_string());
  let task_id = start_task(cx, format!("Editing '{name}'..."));
  let name_clone = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  // Set edit flag on options
  let options = options.with_edit(true);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };

        // Stop the machine first
        if let Err(e) = ColimaClient::stop(name_opt) {
          return Err(format!("Failed to stop machine: {e}"));
        }

        // Start with new options (edit mode)
        match ColimaClient::start(&options) {
          Ok(()) => Ok(ColimaClient::list().unwrap_or_default()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(vms) => {
        state.update(cx, |state, cx| {
          state.set_machines(vms);
          cx.emit(StateChanged::MachinesUpdated);
        });
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Machine '{name_clone}' updated and restarted"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to edit '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn restart_machine(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Restarting '{name}'..."));
  let name_clone = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };
        match ColimaClient::restart(name_opt) {
          Ok(()) => Ok(ColimaClient::list().unwrap_or_default()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(vms) => {
        state.update(cx, |state, cx| {
          state.set_machines(vms);
          cx.emit(StateChanged::MachinesUpdated);
        });
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Machine '{name_clone}' restarted"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to restart '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Start Colima with optional profile name (None = default profile)
pub fn start_colima(profile: Option<&str>, cx: &mut App) {
  let name = profile.unwrap_or("default").to_string();
  start_machine(name, cx);
}

/// Stop Colima with optional profile name (None = default profile)
pub fn stop_colima(profile: Option<&str>, cx: &mut App) {
  let name = profile.unwrap_or("default").to_string();
  stop_machine(name, cx);
}

/// Restart Colima with optional profile name (None = default profile)
pub fn restart_colima(profile: Option<&str>, cx: &mut App) {
  let name = profile.unwrap_or("default").to_string();
  let task_id = start_task(cx, format!("Restarting '{name}'..."));
  let name_clone = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };
        match ColimaClient::restart(name_opt) {
          Ok(()) => Ok(ColimaClient::list().unwrap_or_default()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(vms) => {
        state.update(cx, |state, cx| {
          state.set_machines(vms);
          cx.emit(StateChanged::MachinesUpdated);
        });
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Machine '{name_clone}' restarted"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to restart '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn delete_machine(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting '{name}'..."));
  let name_clone = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };
        match ColimaClient::delete(name_opt, true) {
          Ok(()) => Ok(ColimaClient::list().unwrap_or_default()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(vms) => {
        state.update(cx, |state, cx| {
          state.set_machines(vms);
          cx.emit(StateChanged::MachinesUpdated);
        });
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Machine '{name_clone}' deleted"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

// ==================== Machine Tab Actions ====================

/// Open a machine's terminal tab
pub fn open_machine_terminal(name: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::MachineTabRequest {
      machine_name: name,
      tab: 2, // Terminal is tab 2
    });
  });
}

/// Open a machine's files tab
pub fn open_machine_files(name: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::MachineTabRequest {
      machine_name: name,
      tab: 3, // Files is tab 3
    });
  });
}

// ==================== Kubernetes Actions ====================

/// Start Kubernetes on a Colima machine
pub fn kubernetes_start(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Starting K8s on '{name}'..."));
  let name_clone = name.clone();
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let profile = if name == "default" { None } else { Some(name.as_str()) };
        let mut cmd = std::process::Command::new("colima");
        cmd.arg("kubernetes").arg("start");
        if let Some(p) = profile {
          cmd.arg("--profile").arg(p);
        }
        let output = cmd.output()?;
        if output.status.success() {
          Ok(())
        } else {
          Err(anyhow::anyhow!("{}", String::from_utf8_lossy(&output.stderr)))
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Kubernetes started on '{name_clone}'"),
          });
        });
        refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to start K8s on '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Stop Kubernetes on a Colima machine
pub fn kubernetes_stop(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Stopping K8s on '{name}'..."));
  let name_clone = name.clone();
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let profile = if name == "default" { None } else { Some(name.as_str()) };
        let mut cmd = std::process::Command::new("colima");
        cmd.arg("kubernetes").arg("stop");
        if let Some(p) = profile {
          cmd.arg("--profile").arg(p);
        }
        let output = cmd.output()?;
        if output.status.success() {
          Ok(())
        } else {
          Err(anyhow::anyhow!("{}", String::from_utf8_lossy(&output.stderr)))
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Kubernetes stopped on '{name_clone}'"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to stop K8s on '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Reset Kubernetes on a Colima machine (delete and recreate cluster)
pub fn kubernetes_reset(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Resetting K8s on '{name}'..."));
  let name_clone = name.clone();
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let profile = if name == "default" { None } else { Some(name.as_str()) };
        let mut cmd = std::process::Command::new("colima");
        cmd.arg("kubernetes").arg("reset");
        if let Some(p) = profile {
          cmd.arg("--profile").arg(p);
        }
        let output = cmd.output()?;
        if output.status.success() {
          Ok(())
        } else {
          Err(anyhow::anyhow!("{}", String::from_utf8_lossy(&output.stderr)))
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Kubernetes reset on '{name_clone}'"),
          });
        });
        refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to reset K8s on '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

// ==================== Container Actions ====================

pub fn start_container(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Starting container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.start_container(&id).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container started".to_string(),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to start container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn stop_container(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Stopping container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.stop_container(&id).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container stopped".to_string(),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to stop container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn restart_container(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Restarting container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.restart_container(&id).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container restarted".to_string(),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to restart container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn delete_container(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Deleting container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.remove_container(&id, true).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container deleted".to_string(),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn pause_container(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Pausing container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.pause_container(&id).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container paused".to_string(),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to pause container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn unpause_container(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Resuming container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.unpause_container(&id).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container resumed".to_string(),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to resume container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn kill_container(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Killing container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.kill_container(&id, None).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container killed".to_string(),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to kill container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn rename_container(id: String, new_name: String, cx: &mut App) {
  let task_id = start_task(cx, "Renaming container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.rename_container(&id, &new_name).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container renamed".to_string(),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to rename container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn commit_container(
  id: String,
  repo: String,
  tag: String,
  comment: Option<String>,
  author: Option<String>,
  cx: &mut App,
) {
  let task_id = start_task(cx, "Committing container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker
      .commit_container(&id, &repo, &tag, comment.as_deref(), author.as_deref())
      .await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(image_id)) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Container committed as image: {}", &image_id[..12.min(image_id.len())]),
          });
        });
        refresh_images(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to commit container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn export_container(id: String, output_path: String, cx: &mut App) {
  let task_id = start_task(cx, "Exporting container...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.export_container(&id, &output_path).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Container exported".to_string(),
          });
        });
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to export container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Request to open rename dialog for a container
pub fn request_rename_container(id: String, current_name: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::RenameContainerRequest {
      container_id: id,
      current_name,
    });
  });
}

/// Request to open commit dialog for a container
pub fn request_commit_container(id: String, container_name: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::CommitContainerRequest {
      container_id: id,
      container_name,
    });
  });
}

/// Request to open export dialog for a container
pub fn request_export_container(id: String, container_name: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ExportContainerRequest {
      container_id: id,
      container_name,
    });
  });
}

// Container tab navigation functions
pub fn open_container_terminal(id: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ContainerTabRequest {
      container_id: id,
      tab: 2, // Terminal is tab 2
    });
  });
}

pub fn open_container_logs(id: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ContainerTabRequest {
      container_id: id,
      tab: 1, // Logs is tab 1
    });
  });
}

pub fn open_container_inspect(id: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ContainerTabRequest {
      container_id: id,
      tab: 4, // Inspect is tab 4
    });
  });
}

// Pod tab navigation functions
pub fn open_pod_info(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |state, cx| {
    state.set_view(CurrentView::Pods);
    cx.emit(StateChanged::ViewChanged);
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: 0, // Info is tab 0
    });
  });
}

pub fn open_pod_terminal(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: 2, // Terminal is tab 2
    });
  });
}

pub fn open_pod_logs(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: 1, // Logs is tab 1
    });
  });
}

pub fn open_pod_describe(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: 3, // Describe is tab 3
    });
  });
}

pub fn open_pod_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: 4, // YAML is tab 4
    });
  });
}

// Additional container operations

pub fn open_container_files(id: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ContainerTabRequest {
      container_id: id,
      tab: 3, // Files is tab 3
    });
  });
}

// Additional pod operations

pub fn force_delete_pod(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Force deleting pod {name}..."));
  let disp = dispatcher(cx);
  let name_clone = name.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.force_delete_pod(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Pod {name_clone} force deleted"),
          });
        });
        refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to force delete pod: {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn restart_pod(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Restarting pod {name}..."));
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.restart_pod(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(message) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message });
        });
        refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn create_container(options: crate::ui::containers::CreateContainerOptions, cx: &mut App) {
  let image_name = options.image.clone();
  let start_after = options.start_after_create;
  let task_id = start_task(cx, format!("Creating container from {image_name}..."));

  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;

    // Ensure image exists locally, pull if necessary
    docker
      .ensure_image(&options.image, options.platform.as_docker_arg())
      .await?;

    // Parse command and entrypoint if provided
    let command: Option<Vec<String>> = options
      .command
      .as_ref()
      .map(|c| c.split_whitespace().map(String::from).collect());
    let entrypoint: Option<Vec<String>> = options
      .entrypoint
      .as_ref()
      .map(|e| e.split_whitespace().map(String::from).collect());

    let config = ContainerCreateConfig {
      image: options.image,
      name: options.name,
      platform: options.platform.as_docker_arg().map(String::from),
      command,
      entrypoint,
      working_dir: options.workdir,
      restart_policy: options.restart_policy.as_docker_arg().map(String::from),
      flags: ContainerFlags {
        auto_remove: options.remove_after_stop,
        privileged: options.privileged,
        read_only: options.read_only,
        init: options.docker_init,
      },
      env_vars: options.env_vars,
      ports: options.ports,
      volumes: options.volumes,
      network: options.network,
    };

    let container_id = docker.create_container(config).await?;

    // Start the container if requested
    if start_after {
      docker.start_container(&container_id).await?;
    }

    Ok::<_, anyhow::Error>(())
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Container created from {image_name}"),
          });
        });
        refresh_containers(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to create container: {e}"),
          });
        });
      }
      Err(join_err) => {
        fail_task(cx, task_id, join_err.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Task failed: {join_err}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn refresh_containers(cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    match guard.as_ref() {
      Some(docker) => docker.list_containers(true).await.unwrap_or_default(),
      None => vec![],
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    let containers = result.unwrap_or_default();
    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_containers(containers);
        cx.emit(StateChanged::ContainersUpdated);
      });
    })
  })
  .detach();
}

// ============================================================================
// VOLUME OPERATIONS
// ============================================================================

pub fn create_volume(name: String, driver: String, labels: Vec<(String, String)>, cx: &mut App) {
  let task_id = start_task(cx, format!("Creating volume {name}..."));
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.create_volume_with_opts(&name, &driver, labels).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(_)) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Volume created".to_string(),
          });
        });
        refresh_volumes(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn delete_volume(name: String, cx: &mut App) {
  let task_id = start_task(cx, "Deleting volume...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.remove_volume(&name, true).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Volume deleted".to_string(),
          });
        });
        refresh_volumes(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn refresh_volumes(cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    match guard.as_ref() {
      Some(docker) => docker.list_volumes().await.unwrap_or_default(),
      None => vec![],
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    let volumes = result.unwrap_or_default();
    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_volumes(volumes);
        cx.emit(StateChanged::VolumesUpdated);
      });
    })
  })
  .detach();
}

pub fn list_volume_files(volume_name: String, path: String, cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();
  let volume_name_clone = volume_name.clone();
  let path_clone = path.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.list_volume_files(&volume_name, &path).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |_state, cx| match result {
        Ok(Ok(files)) => {
          cx.emit(StateChanged::VolumeFilesLoaded {
            volume_name: volume_name_clone,
            path: path_clone,
            files,
          });
        }
        Ok(Err(_)) | Err(_) => {
          cx.emit(StateChanged::VolumeFilesError {
            volume_name: volume_name_clone,
          });
        }
      });
    })
  })
  .detach();
}

// ============================================================================
// IMAGE OPERATIONS
// ============================================================================

pub fn refresh_images(cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    match guard.as_ref() {
      Some(docker) => docker.list_images(true).await.unwrap_or_default(),
      None => vec![],
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    let images = result.unwrap_or_default();
    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_images(images);
        cx.emit(StateChanged::ImagesUpdated);
      });
    })
  })
  .detach();
}

pub fn delete_image(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Deleting image...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.remove_image(&id, true).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Image deleted".to_string(),
          });
        });
        refresh_images(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn pull_image(image: String, platform: Option<String>, cx: &mut App) {
  let task_id = start_task(cx, format!("Pulling image {image}..."));
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.pull_image(&image, platform.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Image pulled successfully".to_string(),
          });
        });
        refresh_images(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn inspect_image(image_id: String, cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();
  let image_id_clone = image_id.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;

    // Get image inspect
    let _image = docker.image_inspect(&image_id).await?;

    // Get full inspect data from bollard
    let bollard_docker = docker.client()?;
    let inspect = bollard_docker.inspect_image(&image_id).await?;

    // Parse config
    let config = inspect.config.unwrap_or_default();
    let config_cmd = config.cmd;
    let config_workdir = config.working_dir;
    let config_entrypoint = config.entrypoint;
    let config_exposed_ports: Vec<String> = config
      .exposed_ports
      .map(|p| p.keys().cloned().collect())
      .unwrap_or_default();

    // Parse environment variables
    let config_env: Vec<(String, String)> = config
      .env
      .unwrap_or_default()
      .into_iter()
      .filter_map(|e| {
        let parts: Vec<&str> = e.splitn(2, '=').collect();
        if parts.len() == 2 {
          Some((parts[0].to_string(), parts[1].to_string()))
        } else {
          None
        }
      })
      .collect();

    Ok::<_, anyhow::Error>((
      config_cmd,
      config_workdir,
      config_env,
      config_entrypoint,
      config_exposed_ports,
      image_id,
    ))
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      if let Ok(Ok((config_cmd, config_workdir, config_env, config_entrypoint, config_exposed_ports, _image_id))) =
        result
      {
        // Get containers using this image
        let docker_state_entity = docker_state(cx);
        let containers = docker_state_entity.read(cx).containers.clone();
        let used_by: Vec<String> = containers
          .iter()
          .filter(|c| c.image_id == image_id_clone)
          .map(|c| c.name.clone())
          .collect();

        state.update(cx, |_state, cx| {
          cx.emit(StateChanged::ImageInspectLoaded {
            image_id: image_id_clone,
            data: ImageInspectData {
              config_cmd,
              config_workdir,
              config_env,
              config_entrypoint,
              config_exposed_ports,
              used_by,
            },
          });
        });
      }
    })
  })
  .detach();
}

// ============================================================================
// NETWORK OPERATIONS
// ============================================================================

pub fn refresh_networks(cx: &mut App) {
  let state = docker_state(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.list_networks().await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      if let Ok(Ok(networks)) = result {
        state.update(cx, |state, cx| {
          state.set_networks(networks);
          cx.emit(StateChanged::NetworksUpdated);
        });
      }
    })
  })
  .detach();
}

pub fn create_network(name: String, enable_ipv6: bool, subnet: Option<String>, cx: &mut App) {
  let task_id = start_task(cx, format!("Creating network {name}..."));
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.create_network(&name, enable_ipv6, subnet.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(_)) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Network created".to_string(),
          });
        });
        refresh_networks(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

pub fn delete_network(id: String, cx: &mut App) {
  let task_id = start_task(cx, "Deleting network...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
    docker.remove_network(&id).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| match result {
      Ok(Ok(())) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Network deleted".to_string(),
          });
        });
        refresh_networks(cx);
      }
      Ok(Err(e)) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

// ============================================================================
// MACHINE OPERATIONS
// ============================================================================

pub fn set_view(view: CurrentView, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |state, cx| {
    state.set_view(view);
    cx.emit(StateChanged::ViewChanged);
  });
}

pub fn set_docker_context(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Switching to '{name}'..."));

  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        use std::process::Command;
        // Docker context name for colima is "colima" for default or "colima-<profile>" for others
        let context_name = if name == "default" {
          "colima".to_string()
        } else {
          format!("colima-{name}")
        };

        let output = Command::new("docker").args(["context", "use", &context_name]).output();

        match output {
          Ok(out) if out.status.success() => Ok(context_name),
          Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(context_name) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Docker context switched to '{context_name}'"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to switch context: {e}"),
          });
        });
      }
    })
  })
  .detach();
}

// ============================================================================
// DOCKER COMPOSE OPERATIONS
// ============================================================================

pub fn compose_up(project_name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Starting '{project_name}'..."));
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let project = project_name.clone();
    let result = cx
      .background_executor()
      .spawn(async move {
        use std::process::Command;
        let output = Command::new("docker")
          .args(["compose", "-p", &project, "up", "-d"])
          .output();

        match output {
          Ok(out) if out.status.success() => Ok(()),
          Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Started '{project_name}'"),
          });
        });
        refresh_containers(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to start '{project_name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn compose_down(project_name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Stopping '{project_name}'..."));
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let project = project_name.clone();
    let result = cx
      .background_executor()
      .spawn(async move {
        use std::process::Command;
        let output = Command::new("docker")
          .args(["compose", "-p", &project, "down"])
          .output();

        match output {
          Ok(out) if out.status.success() => Ok(()),
          Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Stopped '{project_name}'"),
          });
        });
        refresh_containers(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to stop '{project_name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn compose_restart(project_name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Restarting '{project_name}'..."));
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let project = project_name.clone();
    let result = cx
      .background_executor()
      .spawn(async move {
        use std::process::Command;
        let output = Command::new("docker")
          .args(["compose", "-p", &project, "restart"])
          .output();

        match output {
          Ok(out) if out.status.success() => Ok(()),
          Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Restarted '{project_name}'"),
          });
        });
        refresh_containers(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to restart '{project_name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

// ============================================================================
// PRUNE OPERATIONS
// ============================================================================

pub fn prune_docker(options: &crate::ui::PruneOptions, cx: &mut App) -> Entity<crate::ui::PruneDialog> {
  use crate::docker::PruneResult;

  let task_id = start_task(cx, "Pruning Docker resources...".to_string());
  let disp = dispatcher(cx);
  let client = docker_client();

  // Create a PruneDialog entity to track results
  let prune_dialog = cx.new(|cx| {
    let mut dialog = crate::ui::PruneDialog::new(cx);
    dialog.set_loading(true);
    dialog
  });
  let prune_dialog_clone = prune_dialog.clone();

  let prune_containers = options.prune_containers;
  let prune_images = options.prune_images;
  let prune_volumes = options.prune_volumes;
  let prune_networks = options.prune_networks;
  let images_dangling_only = options.images_dangling_only;
  let prune_k8s_pods = options.prune_k8s_pods;
  let prune_k8s_pods_all = options.prune_k8s_pods_all;
  let prune_k8s_deployments = options.prune_k8s_deployments;
  let prune_k8s_services = options.prune_k8s_services;

  let tokio_task = Tokio::spawn(cx, async move {
    let guard = client.read().await;
    let docker = guard
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;

    let mut result = PruneResult::default();

    if prune_containers && let Ok(r) = docker.prune_containers().await {
      result.containers_deleted = r.containers_deleted;
      result.space_reclaimed += r.space_reclaimed;
    }

    if prune_images && let Ok(r) = docker.prune_images(images_dangling_only).await {
      result.images_deleted = r.images_deleted;
      result.space_reclaimed += r.space_reclaimed;
    }

    if prune_volumes && let Ok(r) = docker.prune_volumes().await {
      result.volumes_deleted = r.volumes_deleted;
      result.space_reclaimed += r.space_reclaimed;
    }

    if prune_networks && let Ok(r) = docker.prune_networks().await {
      result.networks_deleted = r.networks_deleted;
    }

    // Kubernetes pruning
    let needs_k8s = prune_k8s_pods || prune_k8s_deployments || prune_k8s_services;
    if needs_k8s && let Ok(kube_client) = crate::kubernetes::KubeClient::new().await {
      // Helper to check if namespace is a system namespace
      let is_system_namespace = |ns: &str| matches!(ns, "kube-system" | "kube-public" | "kube-node-lease");

      // Helper to check if resource is a system resource
      let is_system_resource =
        |ns: &str, name: &str| is_system_namespace(ns) || (ns == "default" && name == "kubernetes");

      // Prune deployments first (this will cascade delete their pods)
      if prune_k8s_deployments && let Ok(deployments) = kube_client.list_deployments(None).await {
        for deployment in deployments {
          // Skip system namespaces
          if is_system_namespace(&deployment.namespace) {
            continue;
          }
          if kube_client
            .delete_deployment(&deployment.name, &deployment.namespace)
            .await
            .is_ok()
          {
            result
              .deployments_deleted
              .push(format!("{}/{}", deployment.namespace, deployment.name));
          }
        }
      }

      // Prune services
      if prune_k8s_services && let Ok(services) = kube_client.list_services(None).await {
        for service in services {
          // Skip system namespaces and default kubernetes service
          if is_system_resource(&service.namespace, &service.name) {
            continue;
          }
          if kube_client
            .delete_service(&service.name, &service.namespace)
            .await
            .is_ok()
          {
            result
              .services_deleted
              .push(format!("{}/{}", service.namespace, service.name));
          }
        }
      }

      // Prune pods (only orphans if deployments were pruned, or based on status)
      if prune_k8s_pods && let Ok(pods) = kube_client.list_pods(None).await {
        for pod in pods {
          // Skip system namespaces
          if is_system_namespace(&pod.namespace) {
            continue;
          }

          // If prune_k8s_pods_all is true, delete all pods
          // Otherwise, only delete completed/failed pods
          let should_delete = if prune_k8s_pods_all {
            true
          } else {
            matches!(
              pod.phase,
              crate::kubernetes::PodPhase::Succeeded | crate::kubernetes::PodPhase::Failed
            )
          };

          if should_delete && kube_client.delete_pod(&pod.name, &pod.namespace).await.is_ok() {
            result.pods_deleted.push(format!("{}/{}", pod.namespace, pod.name));
          }
        }
      }
    }

    Ok::<_, anyhow::Error>(result)
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      match result {
        Ok(Ok(prune_result)) => {
          complete_task(cx, task_id);

          let mut parts = Vec::new();
          if !prune_result.containers_deleted.is_empty() {
            parts.push(format!("{} containers", prune_result.containers_deleted.len()));
          }
          if !prune_result.images_deleted.is_empty() {
            parts.push(format!("{} images", prune_result.images_deleted.len()));
          }
          if !prune_result.volumes_deleted.is_empty() {
            parts.push(format!("{} volumes", prune_result.volumes_deleted.len()));
          }
          if !prune_result.networks_deleted.is_empty() {
            parts.push(format!("{} networks", prune_result.networks_deleted.len()));
          }
          if !prune_result.pods_deleted.is_empty() {
            parts.push(format!("{} pods", prune_result.pods_deleted.len()));
          }
          if !prune_result.deployments_deleted.is_empty() {
            parts.push(format!("{} deployments", prune_result.deployments_deleted.len()));
          }
          if !prune_result.services_deleted.is_empty() {
            parts.push(format!("{} services", prune_result.services_deleted.len()));
          }

          let message = if parts.is_empty() {
            "Nothing to prune".to_string()
          } else {
            format!(
              "Pruned: {}. Space reclaimed: {}",
              parts.join(", "),
              prune_result.display_space_reclaimed()
            )
          };

          prune_dialog_clone.update(cx, |dialog, _cx| {
            dialog.set_result(prune_result);
          });

          disp.update(cx, |_, cx| {
            cx.emit(DispatcherEvent::TaskCompleted { message });
          });

          // Refresh all lists
          refresh_containers(cx);
          refresh_images(cx);
          refresh_volumes(cx);
          refresh_networks(cx);
          refresh_pods(cx);
          refresh_services(cx);
          refresh_deployments(cx);
        }
        Ok(Err(e)) => {
          fail_task(cx, task_id, e.to_string());
          prune_dialog_clone.update(cx, |dialog, _cx| {
            dialog.set_error(e.to_string());
          });
          disp.update(cx, |_, cx| {
            cx.emit(DispatcherEvent::TaskFailed {
              error: format!("Failed to prune: {e}"),
            });
          });
        }
        Err(join_err) => {
          fail_task(cx, task_id, join_err.to_string());
          prune_dialog_clone.update(cx, |dialog, _cx| {
            dialog.set_error(join_err.to_string());
          });
          disp.update(cx, |_, cx| {
            cx.emit(DispatcherEvent::TaskFailed {
              error: format!("Task failed: {join_err}"),
            });
          });
        }
      }
    })
  })
  .detach();

  prune_dialog
}

// ==================== Initial Data Loading ====================

pub fn load_initial_data(cx: &mut App) {
  let state = docker_state(cx);
  let client_handle = docker_client();

  // Get saved settings for Docker socket and Colima profile
  let settings = settings_state(cx).read(cx).settings.clone();
  let custom_socket = settings.docker_socket.clone();
  let colima_profile = settings.default_colima_profile.clone();

  // First, get colima VMs and socket path (sync operation)
  let colima_task = cx.background_executor().spawn(async move {
    let vms = ColimaClient::list().unwrap_or_default();

    // Use custom socket if provided, otherwise use colima socket with configured profile
    let socket_path = if custom_socket.is_empty() {
      let profile = if colima_profile == "default" {
        None
      } else {
        Some(colima_profile.as_str())
      };
      ColimaClient::socket_path(profile)
    } else {
      custom_socket
    };
    (vms, socket_path)
  });

  // Then spawn tokio task for Docker operations
  let tokio_task = Tokio::spawn(cx, async move {
    // Wait for colima info
    let (vms, socket_path) = colima_task.await;

    // Initialize the shared Docker client
    let mut new_client = DockerClient::new(socket_path);
    let docker_connected = new_client.connect().await.is_ok();

    // Store in the global if connected
    if docker_connected {
      let mut guard = client_handle.write().await;
      *guard = Some(new_client);
      drop(guard);

      // Now use the shared client for all queries
      let guard = client_handle.read().await;
      let docker = guard.as_ref().unwrap();

      let containers = docker.list_containers(true).await.unwrap_or_default();
      let images = docker.list_images(false).await.unwrap_or_default();
      let volumes = docker.list_volumes().await.unwrap_or_default();
      let networks = docker.list_networks().await.unwrap_or_default();

      (vms, containers, images, volumes, networks)
    } else {
      (vms, vec![], vec![], vec![], vec![])
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    let (vms, containers, images, volumes, networks) = result.unwrap_or_default();

    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_machines(vms);
        state.set_containers(containers);
        state.set_images(images);
        state.set_volumes(volumes);
        state.set_networks(networks);
        state.is_loading = false;
        cx.emit(StateChanged::MachinesUpdated);
        cx.emit(StateChanged::ContainersUpdated);
        cx.emit(StateChanged::ImagesUpdated);
        cx.emit(StateChanged::VolumesUpdated);
        cx.emit(StateChanged::NetworksUpdated);
        cx.emit(StateChanged::Loading);
      });
    })
  })
  .detach();
}

// ============================================================================
// Kubernetes Functions
// ============================================================================

/// Refresh the list of pods
pub fn refresh_pods(cx: &mut App) {
  let state = docker_state(cx);

  let namespace = state.read(cx).selected_namespace.clone();
  let ns_filter = if namespace == "all" { None } else { Some(namespace) };

  let tokio_task = Tokio::spawn(cx, async move {
    match crate::kubernetes::KubeClient::new().await {
      Ok(client) => {
        let pods = client.list_pods(ns_filter.as_deref()).await.unwrap_or_default();
        (true, pods)
      }
      Err(_) => (false, vec![]),
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    let (available, pods) = result.unwrap_or((false, vec![]));

    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_k8s_available(available);
        state.set_pods(pods);
        cx.emit(StateChanged::PodsUpdated);
      });
    })
  })
  .detach();
}

/// Refresh the list of namespaces
pub fn refresh_namespaces(cx: &mut App) {
  let state = docker_state(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    match crate::kubernetes::KubeClient::new().await {
      Ok(client) => {
        let namespaces = client
          .list_namespaces()
          .await
          .unwrap_or_default()
          .into_iter()
          .map(|ns| ns.name)
          .collect();
        (true, namespaces)
      }
      Err(_) => (false, vec!["default".to_string()]),
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    let (available, namespaces) = result.unwrap_or((false, vec!["default".to_string()]));

    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_k8s_available(available);
        state.set_namespaces(namespaces);
        cx.emit(StateChanged::NamespacesUpdated);
      });
    })
  })
  .detach();
}

/// Set the selected namespace for pod filtering
pub fn set_namespace(namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |state, cx| {
    state.set_selected_namespace(namespace);
    cx.emit(StateChanged::NamespacesUpdated);
  });
  // Refresh pods with new namespace filter
  refresh_pods(cx);
}

/// Delete a pod
pub fn delete_pod(name: String, namespace: String, cx: &mut App) {
  let _state = docker_state(cx);
  let disp = dispatcher(cx);
  let task_id = start_task(cx, format!("Deleting pod {name}"));

  let name_clone = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_pod(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Pod {name_clone} deleted"),
          });
        });
        refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed { error: e.to_string() });
        });
      }
    })
  })
  .detach();
}

/// Get logs for a pod
pub fn get_pod_logs(name: String, namespace: String, container: Option<String>, tail_lines: i64, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client
      .get_pod_logs(&name, &namespace, container.as_deref(), Some(tail_lines))
      .await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| {
      let logs = result.unwrap_or_else(|e| format!("Error fetching logs: {e}"));
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::PodLogsLoaded {
          pod_name: name_clone,
          namespace: namespace_clone,
          logs,
        });
      });
    })
  })
  .detach();
}

/// Get pod describe output (kubectl describe pod)
pub fn get_pod_describe(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.describe_pod(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let describe = match result {
      Ok(desc) => desc,
      Err(e) => format!("Error: {e}"),
    };

    cx.update(|cx| {
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::PodDescribeLoaded {
          pod_name: name_clone,
          namespace: namespace_clone,
          describe,
        });
      });
    })
  })
  .detach();
}

/// Get pod YAML manifest (kubectl get pod -o yaml)
pub fn get_pod_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.get_pod_yaml(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let yaml = match result {
      Ok(y) => y,
      Err(e) => format!("Error: {e}"),
    };

    cx.update(|cx| {
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::PodYamlLoaded {
          pod_name: name_clone,
          namespace: namespace_clone,
          yaml,
        });
      });
    })
  })
  .detach();
}

// ==================== Kubernetes Services ====================

/// Refresh services list
pub fn refresh_services(cx: &mut App) {
  let state = docker_state(cx);
  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_services(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| {
      if let Ok(services) = result {
        state.update(cx, |state, cx| {
          state.set_services(services);
          cx.emit(StateChanged::ServicesUpdated);
        });
      }
    })
  })
  .detach();
}

/// Delete a service
pub fn delete_service(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting service '{name}'..."));
  let name_clone = name.clone();
  let _state = docker_state(cx);
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_service(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Service '{name_clone}' deleted"),
          });
        });
        refresh_services(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete service '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Get service YAML
pub fn get_service_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.get_service_yaml(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let yaml = match result {
      Ok(y) => y,
      Err(e) => format!("Error: {e}"),
    };

    cx.update(|cx| {
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::ServiceYamlLoaded {
          service_name: name_clone,
          namespace: namespace_clone,
          yaml,
        });
      });
    })
  })
  .detach();
}

/// Open service with YAML tab selected
pub fn open_service_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ServiceTabRequest {
      service_name: name.clone(),
      namespace: namespace.clone(),
      tab: 2, // YAML tab
    });
  });
  get_service_yaml(name, namespace, cx);
}

/// Create a new Kubernetes service
pub fn create_service(options: crate::kubernetes::CreateServiceOptions, cx: &mut App) {
  let task_id = start_task(cx, format!("Creating service '{}'...", options.name));
  let name = options.name.clone();
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.create_service(options).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(msg) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });
        refresh_services(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to create service '{name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

// ==================== Kubernetes Deployments ====================

/// Refresh deployments list
pub fn refresh_deployments(cx: &mut App) {
  let state = docker_state(cx);
  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_deployments(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| {
      if let Ok(deployments) = result {
        state.update(cx, |state, cx| {
          state.set_deployments(deployments);
          cx.emit(StateChanged::DeploymentsUpdated);
        });
      }
    })
  })
  .detach();
}

/// Delete a deployment
pub fn delete_deployment(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting deployment '{name}'..."));
  let name_clone = name.clone();
  let _state = docker_state(cx);
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_deployment(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Deployment '{name_clone}' deleted"),
          });
        });
        refresh_deployments(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete deployment '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Scale a deployment
pub fn scale_deployment(name: String, namespace: String, replicas: i32, cx: &mut App) {
  let task_id = start_task(cx, format!("Scaling '{name}' to {replicas} replicas..."));
  let name_clone = name.clone();
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.scale_deployment(&name, &namespace, replicas).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(msg) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });
        refresh_deployments(cx);
        refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to scale '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Restart a deployment (rollout restart)
pub fn restart_deployment(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Restarting '{name}'..."));
  let name_clone = name.clone();
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.restart_deployment(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(msg) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });
        refresh_deployments(cx);
        refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to restart '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Get deployment YAML
pub fn get_deployment_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.get_deployment_yaml(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let yaml = match result {
      Ok(y) => y,
      Err(e) => format!("Error: {e}"),
    };

    cx.update(|cx| {
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::DeploymentYamlLoaded {
          deployment_name: name_clone,
          namespace: namespace_clone,
          yaml,
        });
      });
    })
  })
  .detach();
}

/// Open deployment with YAML tab selected
pub fn open_deployment_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::DeploymentTabRequest {
      deployment_name: name.clone(),
      namespace: namespace.clone(),
      tab: 2, // YAML tab
    });
  });
  get_deployment_yaml(name, namespace, cx);
}

/// Create a new Kubernetes deployment
pub fn create_deployment(options: crate::kubernetes::CreateDeploymentOptions, cx: &mut App) {
  let task_id = start_task(cx, format!("Creating deployment '{}'...", options.name));
  let name = options.name.clone();
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.create_deployment(options).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(msg) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });
        refresh_deployments(cx);
        refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to create deployment '{name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Request to open scale dialog for a deployment
pub fn request_scale_dialog(name: String, namespace: String, current_replicas: i32, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::DeploymentScaleRequest {
      deployment_name: name,
      namespace,
      current_replicas,
    });
  });
}
