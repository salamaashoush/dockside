use std::sync::Arc;
use tokio::sync::RwLock;
use gpui::{App, AppContext, Entity, EventEmitter, Global};

use crate::colima::{ColimaClient, ColimaStartOptions};
use crate::docker::DockerClient;
use crate::state::{docker_state, ImageInspectData, StateChanged, CurrentView};
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

// ==================== Initial Data Loading ====================

pub fn load_initial_data(cx: &mut App) {
    let state = docker_state(cx);
    let client_handle = docker_client();

    // First, get colima VMs and socket path (sync operation)
    let colima_task = cx.background_executor().spawn(async move {
        let colima_client = ColimaClient::new();
        let vms = colima_client.list().unwrap_or_default();
        let socket_path = colima_client.socket_path(None);
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
