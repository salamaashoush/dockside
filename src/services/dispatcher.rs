use std::sync::Arc;
use tokio::sync::RwLock;
use gpui::{App, AppContext, Entity, EventEmitter, Global};

use crate::colima::{ColimaClient, ColimaStartOptions};
use crate::docker::DockerClient;
use crate::state::{docker_state, settings_state, ImageInspectData, StateChanged, CurrentView, AppSettings};
use crate::services::{complete_task, fail_task, start_task, Tokio};

/// Shared Docker client - initialized once in load_initial_data
static DOCKER_CLIENT: std::sync::OnceLock<Arc<RwLock<Option<DockerClient>>>> = std::sync::OnceLock::new();

/// Get the shared Docker client handle
pub fn docker_client() -> Arc<RwLock<Option<DockerClient>>> {
    DOCKER_CLIENT
        .get_or_init(|| Arc::new(RwLock::new(None)))
        .clone()
}

/// Event emitted when a task completes (for UI to show notifications)
#[derive(Clone, Debug)]
pub enum DispatcherEvent {
    TaskCompleted { name: String, message: String },
    TaskFailed { name: String, error: String },
}

/// Central action dispatcher - handles all async operations
pub struct ActionDispatcher {
    pub show_create_dialog: bool,
}

impl ActionDispatcher {
    pub fn new() -> Self {
        Self {
            show_create_dialog: false,
        }
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
    let task_id = start_task(cx, "create_machine", format!("Creating '{}'...", machine_name));
    let name_clone = machine_name.clone();

    let state = docker_state(cx);
    let disp = dispatcher(cx);

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move {
                let colima_client = ColimaClient::new();
                match colima_client.start(options) {
                    Ok(_) => Ok(colima_client.list().unwrap_or_default()),
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(vms) => {
                    state.update(cx, |state, cx| {
                        state.set_machines(vms);
                        cx.emit(StateChanged::MachinesUpdated);
                    });
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "create_machine".to_string(),
                            message: format!("Machine '{}' created", name_clone),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_machine".to_string(),
                            error: format!("Failed to create '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn start_machine(name: String, cx: &mut App) {
    let task_id = start_task(cx, "start_machine", format!("Starting '{}'...", name));
    let name_clone = name.clone();

    let state = docker_state(cx);
    let disp = dispatcher(cx);

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move {
                let colima_client = ColimaClient::new();
                let options = ColimaStartOptions::new().with_name(name.clone());
                match colima_client.start(options) {
                    Ok(_) => Ok(colima_client.list().unwrap_or_default()),
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(vms) => {
                    state.update(cx, |state, cx| {
                        state.set_machines(vms);
                        cx.emit(StateChanged::MachinesUpdated);
                    });
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "start_machine".to_string(),
                            message: format!("Machine '{}' started", name_clone),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "start_machine".to_string(),
                            error: format!("Failed to start '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn stop_machine(name: String, cx: &mut App) {
    let task_id = start_task(cx, "stop_machine", format!("Stopping '{}'...", name));
    let name_clone = name.clone();

    let state = docker_state(cx);
    let disp = dispatcher(cx);

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move {
                let colima_client = ColimaClient::new();
                let name_opt = if name == "default" { None } else { Some(name.as_str()) };
                match colima_client.stop(name_opt) {
                    Ok(_) => Ok(colima_client.list().unwrap_or_default()),
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(vms) => {
                    state.update(cx, |state, cx| {
                        state.set_machines(vms);
                        cx.emit(StateChanged::MachinesUpdated);
                    });
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "stop_machine".to_string(),
                            message: format!("Machine '{}' stopped", name_clone),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "stop_machine".to_string(),
                            error: format!("Failed to stop '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn edit_machine(options: ColimaStartOptions, cx: &mut App) {
    let name = options.name.clone().unwrap_or_else(|| "default".to_string());
    let task_id = start_task(cx, "edit_machine", format!("Editing '{}'...", name));
    let name_clone = name.clone();

    let state = docker_state(cx);
    let disp = dispatcher(cx);

    // Set edit flag on options
    let options = options.with_edit(true);

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move {
                let colima_client = ColimaClient::new();
                let name_opt = if name == "default" { None } else { Some(name.as_str()) };

                // Stop the machine first
                if let Err(e) = colima_client.stop(name_opt) {
                    return Err(format!("Failed to stop machine: {}", e));
                }

                // Start with new options (edit mode)
                match colima_client.start(options) {
                    Ok(_) => Ok(colima_client.list().unwrap_or_default()),
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(vms) => {
                    state.update(cx, |state, cx| {
                        state.set_machines(vms);
                        cx.emit(StateChanged::MachinesUpdated);
                    });
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "edit_machine".to_string(),
                            message: format!("Machine '{}' updated and restarted", name_clone),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "edit_machine".to_string(),
                            error: format!("Failed to edit '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn restart_machine(name: String, cx: &mut App) {
    let task_id = start_task(cx, "restart_machine", format!("Restarting '{}'...", name));
    let name_clone = name.clone();

    let state = docker_state(cx);
    let disp = dispatcher(cx);

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move {
                let colima_client = ColimaClient::new();
                let name_opt = if name == "default" { None } else { Some(name.as_str()) };
                match colima_client.restart(name_opt) {
                    Ok(_) => Ok(colima_client.list().unwrap_or_default()),
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(vms) => {
                    state.update(cx, |state, cx| {
                        state.set_machines(vms);
                        cx.emit(StateChanged::MachinesUpdated);
                    });
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "restart_machine".to_string(),
                            message: format!("Machine '{}' restarted", name_clone),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "restart_machine".to_string(),
                            error: format!("Failed to restart '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

/// Start Colima with optional profile name (None = default profile)
pub fn start_colima(profile: Option<String>, cx: &mut App) {
    let name = profile.clone().unwrap_or_else(|| "default".to_string());
    start_machine(name, cx);
}

/// Stop Colima with optional profile name (None = default profile)
pub fn stop_colima(profile: Option<String>, cx: &mut App) {
    let name = profile.unwrap_or_else(|| "default".to_string());
    stop_machine(name, cx);
}

/// Restart Colima with optional profile name (None = default profile)
pub fn restart_colima(profile: Option<String>, cx: &mut App) {
    let name = profile.unwrap_or_else(|| "default".to_string());
    let task_id = start_task(cx, "restart_colima", format!("Restarting '{}'...", name));
    let name_clone = name.clone();

    let state = docker_state(cx);
    let disp = dispatcher(cx);

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move {
                let colima_client = ColimaClient::new();
                let name_opt = if name == "default" { None } else { Some(name.as_str()) };
                match colima_client.restart(name_opt) {
                    Ok(_) => Ok(colima_client.list().unwrap_or_default()),
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(vms) => {
                    state.update(cx, |state, cx| {
                        state.set_machines(vms);
                        cx.emit(StateChanged::MachinesUpdated);
                    });
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "restart_colima".to_string(),
                            message: format!("Machine '{}' restarted", name_clone),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "restart_colima".to_string(),
                            error: format!("Failed to restart '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn delete_machine(name: String, cx: &mut App) {
    let task_id = start_task(cx, "delete_machine", format!("Deleting '{}'...", name));
    let name_clone = name.clone();

    let state = docker_state(cx);
    let disp = dispatcher(cx);

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move {
                let colima_client = ColimaClient::new();
                let name_opt = if name == "default" { None } else { Some(name.as_str()) };
                match colima_client.delete(name_opt, true) {
                    Ok(_) => Ok(colima_client.list().unwrap_or_default()),
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(vms) => {
                    state.update(cx, |state, cx| {
                        state.set_machines(vms);
                        state.clear_selection();
                        cx.emit(StateChanged::MachinesUpdated);
                        cx.emit(StateChanged::SelectionChanged);
                    });
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "delete_machine".to_string(),
                            message: format!("Machine '{}' deleted", name_clone),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_machine".to_string(),
                            error: format!("Failed to delete '{}': {}", name_clone, e),
                        });
                    });
                }
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
    let task_id = start_task(cx, "k8s_start", format!("Starting K8s on '{}'...", name));
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
                    Err(anyhow::anyhow!(
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(_) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "k8s_start".to_string(),
                            message: format!("Kubernetes started on '{}'", name_clone),
                        });
                    });
                    refresh_pods(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "k8s_start".to_string(),
                            error: format!("Failed to start K8s on '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

/// Stop Kubernetes on a Colima machine
pub fn kubernetes_stop(name: String, cx: &mut App) {
    let task_id = start_task(cx, "k8s_stop", format!("Stopping K8s on '{}'...", name));
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
                    Err(anyhow::anyhow!(
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(_) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "k8s_stop".to_string(),
                            message: format!("Kubernetes stopped on '{}'", name_clone),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "k8s_stop".to_string(),
                            error: format!("Failed to stop K8s on '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

/// Reset Kubernetes on a Colima machine (delete and recreate cluster)
pub fn kubernetes_reset(name: String, cx: &mut App) {
    let task_id = start_task(cx, "k8s_reset", format!("Resetting K8s on '{}'...", name));
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
                    Err(anyhow::anyhow!(
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(_) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "k8s_reset".to_string(),
                            message: format!("Kubernetes reset on '{}'", name_clone),
                        });
                    });
                    refresh_pods(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "k8s_reset".to_string(),
                            error: format!("Failed to reset K8s on '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

// ==================== Container Actions ====================

pub fn start_container(id: String, cx: &mut App) {
    let task_id = start_task(cx, "start_container", "Starting container...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "start_container".to_string(),
                            message: "Container started".to_string(),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "start_container".to_string(),
                            error: format!("Failed to start container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "start_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn stop_container(id: String, cx: &mut App) {
    let task_id = start_task(cx, "stop_container", "Stopping container...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "stop_container".to_string(),
                            message: "Container stopped".to_string(),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "stop_container".to_string(),
                            error: format!("Failed to stop container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "stop_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn restart_container(id: String, cx: &mut App) {
    let task_id = start_task(cx, "restart_container", "Restarting container...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "restart_container".to_string(),
                            message: "Container restarted".to_string(),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "restart_container".to_string(),
                            error: format!("Failed to restart container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "restart_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn delete_container(id: String, cx: &mut App) {
    let task_id = start_task(cx, "delete_container", "Deleting container...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "delete_container".to_string(),
                            message: "Container deleted".to_string(),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_container".to_string(),
                            error: format!("Failed to delete container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn pause_container(id: String, cx: &mut App) {
    let task_id = start_task(cx, "pause_container", "Pausing container...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "pause_container".to_string(),
                            message: "Container paused".to_string(),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "pause_container".to_string(),
                            error: format!("Failed to pause container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "pause_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn unpause_container(id: String, cx: &mut App) {
    let task_id = start_task(cx, "unpause_container", "Resuming container...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "unpause_container".to_string(),
                            message: "Container resumed".to_string(),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "unpause_container".to_string(),
                            error: format!("Failed to resume container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "unpause_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn kill_container(id: String, cx: &mut App) {
    let task_id = start_task(cx, "kill_container", "Killing container...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "kill_container".to_string(),
                            message: "Container killed".to_string(),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "kill_container".to_string(),
                            error: format!("Failed to kill container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "kill_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
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
            tab: 3, // Inspect is tab 3
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

pub fn rename_container(id: String, new_name: String, cx: &mut App) {
    let task_id = start_task(cx, "rename_container", format!("Renaming container to {}...", new_name));
    let disp = dispatcher(cx);
    let client = docker_client();
    let new_name_clone = new_name.clone();

    let tokio_task = Tokio::spawn(cx, async move {
        let guard = client.read().await;
        let docker = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
        docker.rename_container(&id, &new_name).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await;
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "rename_container".to_string(),
                            message: format!("Container renamed to {}", new_name_clone),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "rename_container".to_string(),
                            error: format!("Failed to rename container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "rename_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn commit_container(id: String, repo: String, tag: String, cx: &mut App) {
    let image_name = format!("{}:{}", repo, tag);
    let task_id = start_task(cx, "commit_container", format!("Creating image {}...", image_name));
    let disp = dispatcher(cx);
    let client = docker_client();
    let image_name_clone = image_name.clone();

    let tokio_task = Tokio::spawn(cx, async move {
        let guard = client.read().await;
        let docker = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
        docker.commit_container(&id, &repo, &tag, None, None).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await;
        cx.update(|cx| {
            match result {
                Ok(Ok(_image_id)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "commit_container".to_string(),
                            message: format!("Image {} created from container", image_name_clone),
                        });
                    });
                    refresh_images(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "commit_container".to_string(),
                            error: format!("Failed to commit container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "commit_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn export_container(id: String, output_path: String, cx: &mut App) {
    let task_id = start_task(cx, "export_container", format!("Exporting container to {}...", output_path));
    let disp = dispatcher(cx);
    let client = docker_client();
    let output_clone = output_path.clone();

    let tokio_task = Tokio::spawn(cx, async move {
        let guard = client.read().await;
        let docker = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;
        docker.export_container(&id, &output_path).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await;
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "export_container".to_string(),
                            message: format!("Container exported to {}", output_clone),
                        });
                    });
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "export_container".to_string(),
                            error: format!("Failed to export container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "export_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn open_container_files(id: String, cx: &mut App) {
    let state = docker_state(cx);
    state.update(cx, |_state, cx| {
        cx.emit(StateChanged::ContainerTabRequest {
            container_id: id,
            tab: 3, // Files is tab 3
        });
    });
}

pub fn open_container_stats(id: String, cx: &mut App) {
    let state = docker_state(cx);
    state.update(cx, |_state, cx| {
        cx.emit(StateChanged::ContainerTabRequest {
            container_id: id,
            tab: 4, // Stats is tab 4
        });
    });
}

// Additional pod operations

pub fn force_delete_pod(name: String, namespace: String, cx: &mut App) {
    let task_id = start_task(cx, "force_delete_pod", format!("Force deleting pod {}...", name));
    let disp = dispatcher(cx);
    let name_clone = name.clone();

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.force_delete_pod(&name, &namespace).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));
        cx.update(|cx| {
            match result {
                Ok(_) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "force_delete_pod".to_string(),
                            message: format!("Pod {} force deleted", name_clone),
                        });
                    });
                    refresh_pods(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "force_delete_pod".to_string(),
                            error: format!("Failed to force delete pod: {}", e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn restart_pod(name: String, namespace: String, cx: &mut App) {
    let task_id = start_task(cx, "restart_pod", format!("Restarting pod {}...", name));
    let disp = dispatcher(cx);

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.restart_pod(&name, &namespace).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));
        cx.update(|cx| {
            match result {
                Ok(message) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "restart_pod".to_string(),
                            message,
                        });
                    });
                    refresh_pods(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "restart_pod".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn port_forward_pod(name: String, namespace: String, local_port: u16, remote_port: u16, cx: &mut App) {
    let task_id = start_task(
        cx,
        "port_forward_pod",
        format!("Port forwarding {}:{} -> pod {}:{}", local_port, remote_port, name, remote_port),
    );
    let disp = dispatcher(cx);

    // Start kubectl port-forward as a background process
    let tokio_task = Tokio::spawn(cx, async move {
        let mut child = tokio::process::Command::new("kubectl")
            .args([
                "port-forward",
                &format!("pod/{}", name),
                "-n",
                &namespace,
                &format!("{}:{}", local_port, remote_port),
            ])
            .spawn()?;

        // Wait a bit to see if it fails immediately
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        match child.try_wait()? {
            Some(status) => {
                if status.success() {
                    Ok::<_, anyhow::Error>((local_port, remote_port))
                } else {
                    Err(anyhow::anyhow!("Port forward process exited with status: {}", status))
                }
            }
            None => {
                // Process is still running, which is good
                Ok((local_port, remote_port))
            }
        }
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await;
        cx.update(|cx| {
            match result {
                Ok(Ok((local, remote))) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "port_forward_pod".to_string(),
                            message: format!("Port forward active: localhost:{} -> pod:{}", local, remote),
                        });
                    });
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "port_forward_pod".to_string(),
                            error: format!("Failed to port forward: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "port_forward_pod".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn create_container(options: crate::ui::containers::CreateContainerOptions, cx: &mut App) {
    let image_name = options.image.clone();
    let start_after = options.start_after_create;
    let task_id = start_task(
        cx,
        "create_container",
        format!("Creating container from {}...", image_name),
    );

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

        let container_id = docker
            .create_container(
                &options.image,
                options.name.as_deref(),
                options.platform.as_docker_arg(),
                command,
                entrypoint,
                options.workdir.as_deref(),
                options.remove_after_stop,
                options.restart_policy.as_docker_arg(),
                options.privileged,
                options.read_only,
                options.docker_init,
                options.env_vars,
                options.ports,
                options.volumes,
                options.network.as_deref(),
            )
            .await?;

        // Start the container if requested
        if start_after {
            docker.start_container(&container_id).await?;
        }

        Ok::<_, anyhow::Error>(())
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await;
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "create_container".to_string(),
                            message: format!("Container created from {}", image_name),
                        });
                    });
                    refresh_containers(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_container".to_string(),
                            error: format!("Failed to create container: {}", e),
                        });
                    });
                }
                Err(join_err) => {
                    fail_task(cx, task_id, join_err.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_container".to_string(),
                            error: format!("Task failed: {}", join_err),
                        });
                    });
                }
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

pub fn create_volume(
    name: String,
    driver: String,
    labels: Vec<(String, String)>,
    cx: &mut App,
) {
    let task_id = start_task(cx, "create_volume", format!("Creating volume {}...", name));
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "create_volume".to_string(),
                            message: "Volume created".to_string(),
                        });
                    });
                    refresh_volumes(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_volume".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_volume".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn delete_volume(name: String, cx: &mut App) {
    let task_id = start_task(cx, "delete_volume", "Deleting volume...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "delete_volume".to_string(),
                            message: "Volume deleted".to_string(),
                        });
                    });
                    refresh_volumes(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_volume".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_volume".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
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
            state.update(cx, |_state, cx| {
                match result {
                    Ok(Ok(files)) => {
                        cx.emit(StateChanged::VolumeFilesLoaded {
                            volume_name: volume_name_clone,
                            path: path_clone,
                            files,
                        });
                    }
                    Ok(Err(e)) => {
                        cx.emit(StateChanged::VolumeFilesError {
                            volume_name: volume_name_clone,
                            error: e.to_string(),
                        });
                    }
                    Err(e) => {
                        cx.emit(StateChanged::VolumeFilesError {
                            volume_name: volume_name_clone,
                            error: e.to_string(),
                        });
                    }
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
    let task_id = start_task(cx, "delete_image", "Deleting image...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "delete_image".to_string(),
                            message: "Image deleted".to_string(),
                        });
                    });
                    refresh_images(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_image".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_image".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn pull_image(image: String, platform: Option<String>, cx: &mut App) {
    let task_id = start_task(cx, "pull_image", format!("Pulling image {}...", image));
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "pull_image".to_string(),
                            message: "Image pulled successfully".to_string(),
                        });
                    });
                    refresh_images(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "pull_image".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "pull_image".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
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

        Ok::<_, anyhow::Error>((config_cmd, config_workdir, config_env, config_entrypoint, config_exposed_ports, image_id))
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await;
        cx.update(|cx| {
            if let Ok(Ok((config_cmd, config_workdir, config_env, config_entrypoint, config_exposed_ports, _image_id))) = result {
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
    let task_id = start_task(cx, "create_network", format!("Creating network {}...", name));
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "create_network".to_string(),
                            message: "Network created".to_string(),
                        });
                    });
                    refresh_networks(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_network".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_network".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn delete_network(id: String, cx: &mut App) {
    let task_id = start_task(cx, "delete_network", "Deleting network...".to_string());
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
        cx.update(|cx| {
            match result {
                Ok(Ok(_)) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "delete_network".to_string(),
                            message: "Network deleted".to_string(),
                        });
                    });
                    refresh_networks(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_network".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_network".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

// ============================================================================
// MACHINE OPERATIONS
// ============================================================================

pub fn refresh_machines(cx: &mut App) {
    let state = docker_state(cx);

    cx.spawn(async move |cx| {
        let vms = cx
            .background_executor()
            .spawn(async move {
                let colima_client = ColimaClient::new();
                colima_client.list().unwrap_or_default()
            })
            .await;

        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.set_machines(vms);
                cx.emit(StateChanged::MachinesUpdated);
            });
        })
    })
    .detach();
}

pub fn select_machine(name: String, cx: &mut App) {
    let state = docker_state(cx);
    state.update(cx, |state, cx| {
        state.select_machine(&name);
        cx.emit(StateChanged::SelectionChanged);
    });
}

pub fn set_view(view: CurrentView, cx: &mut App) {
    let state = docker_state(cx);
    state.update(cx, |state, cx| {
        state.set_view(view);
        cx.emit(StateChanged::ViewChanged);
    });
}

pub fn show_create_dialog(cx: &mut App) {
    let disp = dispatcher(cx);
    disp.update(cx, |d, _| {
        d.show_create_dialog = true;
    });
}

pub fn hide_create_dialog(cx: &mut App) {
    let disp = dispatcher(cx);
    disp.update(cx, |d, _| {
        d.show_create_dialog = false;
    });
}

pub fn set_docker_context(name: String, cx: &mut App) {
    let task_id = start_task(cx, "set_context", format!("Switching to '{}'...", name));

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
                    format!("colima-{}", name)
                };

                let output = Command::new("docker")
                    .args(["context", "use", &context_name])
                    .output();

                match output {
                    Ok(out) if out.status.success() => Ok(context_name),
                    Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        cx.update(|cx| {
            match result {
                Ok(context_name) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "set_context".to_string(),
                            message: format!("Docker context switched to '{}'", context_name),
                        });
                    });
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "set_context".to_string(),
                            error: format!("Failed to switch context: {}", e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

// ============================================================================
// CONTAINER SELECTION
// ============================================================================

pub fn select_container(container_id: String, cx: &mut App) {
    let state = docker_state(cx);
    state.update(cx, |state, cx| {
        state.select_container(&container_id);
        cx.emit(StateChanged::SelectionChanged);
    });
}

// ============================================================================
// DOCKER COMPOSE OPERATIONS
// ============================================================================

pub fn compose_up(project_name: String, cx: &mut App) {
    let task_id = start_task(cx, "compose_up", format!("Starting '{}'...", project_name));
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

        cx.update(|cx| {
            match result {
                Ok(()) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "compose_up".to_string(),
                            message: format!("Started '{}'", project_name),
                        });
                    });
                    refresh_containers(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "compose_up".to_string(),
                            error: format!("Failed to start '{}': {}", project_name, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn compose_down(project_name: String, cx: &mut App) {
    let task_id = start_task(cx, "compose_down", format!("Stopping '{}'...", project_name));
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

        cx.update(|cx| {
            match result {
                Ok(()) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "compose_down".to_string(),
                            message: format!("Stopped '{}'", project_name),
                        });
                    });
                    refresh_containers(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "compose_down".to_string(),
                            error: format!("Failed to stop '{}': {}", project_name, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

pub fn compose_restart(project_name: String, cx: &mut App) {
    let task_id = start_task(cx, "compose_restart", format!("Restarting '{}'...", project_name));
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

        cx.update(|cx| {
            match result {
                Ok(()) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "compose_restart".to_string(),
                            message: format!("Restarted '{}'", project_name),
                        });
                    });
                    refresh_containers(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.clone());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "compose_restart".to_string(),
                            error: format!("Failed to restart '{}': {}", project_name, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

// ============================================================================
// PRUNE OPERATIONS
// ============================================================================

pub fn prune_docker(options: crate::ui::PruneOptions, cx: &mut App) -> Entity<crate::ui::PruneDialog> {
    use crate::docker::PruneResult;

    let task_id = start_task(cx, "prune", "Pruning Docker resources...".to_string());
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

    let tokio_task = Tokio::spawn(cx, async move {
        let guard = client.read().await;
        let docker = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Docker client not connected"))?;

        let mut result = PruneResult::default();

        if prune_containers {
            if let Ok(r) = docker.prune_containers().await {
                result.containers_deleted = r.containers_deleted;
                result.space_reclaimed += r.space_reclaimed;
            }
        }

        if prune_images {
            if let Ok(r) = docker.prune_images(images_dangling_only).await {
                result.images_deleted = r.images_deleted;
                result.space_reclaimed += r.space_reclaimed;
            }
        }

        if prune_volumes {
            if let Ok(r) = docker.prune_volumes().await {
                result.volumes_deleted = r.volumes_deleted;
                result.space_reclaimed += r.space_reclaimed;
            }
        }

        if prune_networks {
            if let Ok(r) = docker.prune_networks().await {
                result.networks_deleted = r.networks_deleted;
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

                    let message = format!(
                        "Pruned: {} containers, {} images, {} volumes, {} networks. Space reclaimed: {}",
                        prune_result.containers_deleted.len(),
                        prune_result.images_deleted.len(),
                        prune_result.volumes_deleted.len(),
                        prune_result.networks_deleted.len(),
                        prune_result.display_space_reclaimed()
                    );

                    prune_dialog_clone.update(cx, |dialog, _cx| {
                        dialog.set_result(prune_result);
                    });

                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "prune".to_string(),
                            message,
                        });
                    });

                    // Refresh all lists
                    refresh_containers(cx);
                    refresh_images(cx);
                    refresh_volumes(cx);
                    refresh_networks(cx);
                }
                Ok(Err(e)) => {
                    fail_task(cx, task_id, e.to_string());
                    prune_dialog_clone.update(cx, |dialog, _cx| {
                        dialog.set_error(e.to_string());
                    });
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "prune".to_string(),
                            error: format!("Failed to prune: {}", e),
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
                            name: "prune".to_string(),
                            error: format!("Task failed: {}", join_err),
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
        let colima_client = ColimaClient::new();
        let vms = colima_client.list().unwrap_or_default();

        // Use custom socket if provided, otherwise use colima socket with configured profile
        let socket_path = if !custom_socket.is_empty() {
            custom_socket
        } else {
            let profile = if colima_profile == "default" { None } else { Some(colima_profile.as_str()) };
            colima_client.socket_path(profile)
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
                cx.emit(StateChanged::Loading(false));
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
                let pods = client
                    .list_pods(ns_filter.as_deref())
                    .await
                    .unwrap_or_default();
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
    let state = docker_state(cx);
    let disp = dispatcher(cx);
    let task_id = start_task(cx, "delete_pod", &format!("Deleting pod {}", name));

    let name_clone = name.clone();
    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.delete_pod(&name, &namespace).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

        cx.update(|cx| {
            match result {
                Ok(()) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "delete_pod".to_string(),
                            message: format!("Pod {} deleted", name_clone),
                        });
                    });
                    refresh_pods(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_pod".to_string(),
                            error: e.to_string(),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

/// Get logs for a pod
pub fn get_pod_logs(
    name: String,
    namespace: String,
    container: Option<String>,
    tail_lines: i64,
    cx: &mut App,
) {
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
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

        cx.update(|cx| {
            let logs = result.unwrap_or_else(|e| format!("Error fetching logs: {}", e));
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
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));
        let describe = match result {
            Ok(desc) => desc,
            Err(e) => format!("Error: {}", e),
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
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));
        let yaml = match result {
            Ok(y) => y,
            Err(e) => format!("Error: {}", e),
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
    let namespace = if selected_ns == "all" {
        None
    } else {
        Some(selected_ns)
    };

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.list_services(namespace.as_deref()).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

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
    let task_id = start_task(cx, "delete_service", format!("Deleting service '{}'...", name));
    let name_clone = name.clone();
    let state = docker_state(cx);
    let disp = dispatcher(cx);

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.delete_service(&name, &namespace).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

        cx.update(|cx| {
            match result {
                Ok(_) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "delete_service".to_string(),
                            message: format!("Service '{}' deleted", name_clone),
                        });
                    });
                    refresh_services(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_service".to_string(),
                            error: format!("Failed to delete service '{}': {}", name_clone, e),
                        });
                    });
                }
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
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));
        let yaml = match result {
            Ok(y) => y,
            Err(e) => format!("Error: {}", e),
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
    let task_id = start_task(
        cx,
        "create_service",
        format!("Creating service '{}'...", options.name),
    );
    let name = options.name.clone();
    let disp = dispatcher(cx);

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.create_service(options).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

        cx.update(|cx| {
            match result {
                Ok(msg) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "create_service".to_string(),
                            message: msg,
                        });
                    });
                    refresh_services(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_service".to_string(),
                            error: format!("Failed to create service '{}': {}", name, e),
                        });
                    });
                }
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
    let namespace = if selected_ns == "all" {
        None
    } else {
        Some(selected_ns)
    };

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.list_deployments(namespace.as_deref()).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

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
    let task_id = start_task(cx, "delete_deployment", format!("Deleting deployment '{}'...", name));
    let name_clone = name.clone();
    let state = docker_state(cx);
    let disp = dispatcher(cx);

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.delete_deployment(&name, &namespace).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

        cx.update(|cx| {
            match result {
                Ok(_) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "delete_deployment".to_string(),
                            message: format!("Deployment '{}' deleted", name_clone),
                        });
                    });
                    refresh_deployments(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "delete_deployment".to_string(),
                            error: format!("Failed to delete deployment '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

/// Scale a deployment
pub fn scale_deployment(name: String, namespace: String, replicas: i32, cx: &mut App) {
    let task_id = start_task(
        cx,
        "scale_deployment",
        format!("Scaling '{}' to {} replicas...", name, replicas),
    );
    let name_clone = name.clone();
    let disp = dispatcher(cx);

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.scale_deployment(&name, &namespace, replicas).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

        cx.update(|cx| {
            match result {
                Ok(msg) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "scale_deployment".to_string(),
                            message: msg,
                        });
                    });
                    refresh_deployments(cx);
                    refresh_pods(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "scale_deployment".to_string(),
                            error: format!("Failed to scale '{}': {}", name_clone, e),
                        });
                    });
                }
            }
        })
    })
    .detach();
}

/// Restart a deployment (rollout restart)
pub fn restart_deployment(name: String, namespace: String, cx: &mut App) {
    let task_id = start_task(cx, "restart_deployment", format!("Restarting '{}'...", name));
    let name_clone = name.clone();
    let disp = dispatcher(cx);

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.restart_deployment(&name, &namespace).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

        cx.update(|cx| {
            match result {
                Ok(msg) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "restart_deployment".to_string(),
                            message: msg,
                        });
                    });
                    refresh_deployments(cx);
                    refresh_pods(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "restart_deployment".to_string(),
                            error: format!("Failed to restart '{}': {}", name_clone, e),
                        });
                    });
                }
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
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));
        let yaml = match result {
            Ok(y) => y,
            Err(e) => format!("Error: {}", e),
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
    let task_id = start_task(
        cx,
        "create_deployment",
        format!("Creating deployment '{}'...", options.name),
    );
    let name = options.name.clone();
    let disp = dispatcher(cx);

    let tokio_task = Tokio::spawn(cx, async move {
        let client = crate::kubernetes::KubeClient::new().await?;
        client.create_deployment(options).await
    });

    cx.spawn(async move |cx| {
        let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));

        cx.update(|cx| {
            match result {
                Ok(msg) => {
                    complete_task(cx, task_id);
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskCompleted {
                            name: "create_deployment".to_string(),
                            message: msg,
                        });
                    });
                    refresh_deployments(cx);
                    refresh_pods(cx);
                }
                Err(e) => {
                    fail_task(cx, task_id, e.to_string());
                    disp.update(cx, |_, cx| {
                        cx.emit(DispatcherEvent::TaskFailed {
                            name: "create_deployment".to_string(),
                            error: format!("Failed to create deployment '{}': {}", name, e),
                        });
                    });
                }
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
