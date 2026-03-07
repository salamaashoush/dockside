//! Core dispatcher types and Docker client management

use gpui::{App, AppContext, Entity, EventEmitter, Global};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::colima::MachineId;
use crate::docker::DockerClient;
use crate::platform::DockerRuntime;
use crate::state::{StateChanged, docker_state};

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

/// Switch the Docker runtime to a different machine
///
/// This disconnects the current Docker client and connects to a new runtime.
/// Used when switching between Host Docker and Colima VMs.
pub fn switch_runtime(machine_id: MachineId, cx: &mut App) {
  let state = docker_state(cx);
  let disp = dispatcher(cx);
  let client_handle = docker_client();

  // Determine the runtime to use based on machine_id
  let runtime = match &machine_id {
    MachineId::Host => DockerRuntime::native_default(),
    MachineId::Colima(profile) => DockerRuntime::Colima {
      profile: profile.clone(),
    },
  };

  let machine_id_clone = machine_id.clone();

  cx.spawn(async move |cx| {
    // Create and connect to the new runtime
    let mut new_client = DockerClient::new(runtime);
    let connected = new_client.connect().await.is_ok();

    if connected {
      // Update the shared client
      let mut guard = client_handle.write().await;
      *guard = Some(new_client);
      drop(guard);

      // Update state with the new active machine
      cx.update(|cx| {
        state.update(cx, |state, cx| {
          state.set_active(machine_id_clone.clone());
          cx.emit(StateChanged::RuntimeSwitched {
            machine_id: machine_id_clone,
          });
        });
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Runtime switched successfully".to_string(),
          });
        });
      })
    } else {
      cx.update(|cx| {
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: "Failed to connect to Docker runtime".to_string(),
          });
        });
      })
    }
  })
  .detach();
}
