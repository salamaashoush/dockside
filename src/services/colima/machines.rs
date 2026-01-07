//! Colima machine operations

use gpui::App;

use crate::colima::{ColimaClient, ColimaConfig};
use crate::services::{TaskStage, advance_stage, complete_task, fail_task, start_staged_task, start_task};
use crate::state::{StateChanged, docker_state};
use crate::utils::{docker_cmd, kubectl_cmd};

use super::super::core::{DispatcherEvent, dispatcher};
use super::super::docker::refresh_containers;
use super::super::kubernetes::{refresh_deployments, refresh_namespaces, refresh_pods, refresh_services};

/// Create a new machine using the config file approach
pub fn create_machine(profile: String, config: ColimaConfig, cx: &mut App) {
  let has_kubernetes = config.kubernetes.enabled;

  // Create staged task with clear progress stages
  let mut stages = vec![
    TaskStage::new("Writing configuration..."),
    TaskStage::new("Downloading VM image..."),
    TaskStage::new(format!("Creating VM '{profile}'...")),
    TaskStage::new("Configuring runtime..."),
  ];

  if has_kubernetes {
    stages.push(TaskStage::new("Setting up Kubernetes..."));
  }

  stages.push(TaskStage::new("Verifying machine..."));

  let task_id = start_staged_task(cx, format!("Creating '{profile}'"), stages);
  let profile_clone = profile.clone();
  let profile_for_context = profile.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        // Start using config file approach
        match ColimaClient::start_with_config(&profile, &config) {
          Ok(()) => {
            let vms = ColimaClient::list().unwrap_or_default();

            // If kubernetes is enabled, switch kubectl context
            if has_kubernetes {
              let kubectl_context = if profile == "default" {
                "colima".to_string()
              } else {
                format!("colima-{profile}")
              };
              // Try to switch kubectl context (don't fail if it doesn't work)
              let _ = kubectl_cmd().args(["config", "use-context", &kubectl_context]).output();
            }

            Ok(vms)
          }
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
            message: format!("Machine '{profile_clone}' created"),
          });
        });
        // Refresh K8s data if kubernetes was enabled
        if has_kubernetes {
          refresh_pods(cx);
          refresh_namespaces(cx);
          refresh_services(cx);
          refresh_deployments(cx);
        }
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to create '{profile_for_context}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Edit an existing machine using the config file approach
pub fn edit_machine(profile: String, config: ColimaConfig, cx: &mut App) {
  let has_kubernetes = config.kubernetes.enabled;

  // Create staged task with clear progress stages
  let stages = vec![
    TaskStage::new(format!("Stopping '{profile}'...")),
    TaskStage::new("Writing new configuration..."),
    TaskStage::new(format!("Starting '{profile}' with new configuration...")),
    TaskStage::new(format!("Verifying '{profile}'...")),
  ];

  let task_id = start_staged_task(cx, format!("Updating '{profile}'"), stages);
  let profile_clone = profile.clone();
  let profile_for_context = profile.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    // Stage 0: Stop the machine
    let stop_result = cx
      .background_executor()
      .spawn({
        let profile = profile.clone();
        async move {
          let profile_opt = if profile == "default" {
            None
          } else {
            Some(profile.as_str())
          };
          ColimaClient::stop(profile_opt)
        }
      })
      .await;

    if let Err(e) = stop_result {
      cx.update(|cx| {
        fail_task(cx, task_id, format!("Failed to stop: {e}"));
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to stop '{profile_clone}': {e}"),
          });
        });
      })
      .ok();
      return;
    }

    // Stage 1: Write new configuration
    cx.update(|cx| advance_stage(cx, task_id)).ok();

    // Brief pause to let Colima release resources
    cx.background_executor()
      .timer(std::time::Duration::from_millis(500))
      .await;

    // Stage 2: Start with new config
    cx.update(|cx| advance_stage(cx, task_id)).ok();

    let start_result = cx
      .background_executor()
      .spawn({
        let profile = profile.clone();
        let config = config.clone();
        async move { ColimaClient::start_with_config(&profile, &config) }
      })
      .await;

    if let Err(e) = start_result {
      cx.update(|cx| {
        fail_task(cx, task_id, format!("Failed to start: {e}"));
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to start '{profile_clone}' with new settings: {e}"),
          });
        });
      })
      .ok();
      return;
    }

    // Stage 3: Verify and refresh list
    cx.update(|cx| advance_stage(cx, task_id)).ok();

    let profile_for_k8s = profile.clone();
    let vms = cx
      .background_executor()
      .spawn(async move {
        let vms = ColimaClient::list().unwrap_or_default();

        // If kubernetes was enabled, switch kubectl context
        if has_kubernetes {
          let kubectl_context = if profile_for_k8s == "default" {
            "colima".to_string()
          } else {
            format!("colima-{profile_for_k8s}")
          };
          // Try to switch kubectl context (don't fail if it doesn't work)
          let _ = kubectl_cmd().args(["config", "use-context", &kubectl_context]).output();
        }

        vms
      })
      .await;

    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_machines(vms);
        cx.emit(StateChanged::MachinesUpdated);
      });
      complete_task(cx, task_id);
      disp.update(cx, |_, cx| {
        cx.emit(DispatcherEvent::TaskCompleted {
          message: format!("Machine '{profile_for_context}' updated successfully"),
        });
      });
      // Refresh K8s data if kubernetes was enabled
      if has_kubernetes {
        refresh_pods(cx);
        refresh_namespaces(cx);
        refresh_services(cx);
        refresh_deployments(cx);
      }
    })
    .ok();
  })
  .detach();
}

/// Start an existing machine (uses existing config)
pub fn start_machine(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Starting '{name}'..."));
  let name_clone = name.clone();
  let name_for_context = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };
        match ColimaClient::start_existing(name_opt) {
          Ok(()) => {
            let vms = ColimaClient::list().unwrap_or_default();
            // Check if the started machine has kubernetes enabled
            let has_k8s = vms.iter().any(|vm| vm.name == name && vm.kubernetes);

            // If kubernetes is enabled, switch kubectl context
            if has_k8s {
              let kubectl_context = if name == "default" {
                "colima".to_string()
              } else {
                format!("colima-{name}")
              };
              // Try to switch kubectl context (don't fail if it doesn't work)
              let _ = kubectl_cmd().args(["config", "use-context", &kubectl_context]).output();
            }

            Ok((vms, has_k8s))
          }
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok((vms, has_k8s)) => {
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
        // Refresh K8s data if kubernetes is enabled
        if has_k8s {
          refresh_pods(cx);
          refresh_namespaces(cx);
          refresh_services(cx);
          refresh_deployments(cx);
        }
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to start '{name_for_context}': {e}"),
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

pub fn restart_machine(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Restarting '{name}'..."));
  let name_clone = name.clone();
  let name_for_context = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };
        match ColimaClient::restart(name_opt) {
          Ok(()) => {
            let vms = ColimaClient::list().unwrap_or_default();
            // Check if the restarted machine has kubernetes enabled
            let has_k8s = vms.iter().any(|vm| vm.name == name && vm.kubernetes);

            // If kubernetes is enabled, switch kubectl context
            if has_k8s {
              let kubectl_context = if name == "default" {
                "colima".to_string()
              } else {
                format!("colima-{name}")
              };
              // Try to switch kubectl context (don't fail if it doesn't work)
              let _ = kubectl_cmd().args(["config", "use-context", &kubectl_context]).output();
            }

            Ok((vms, has_k8s))
          }
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok((vms, has_k8s)) => {
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
        // Refresh K8s data if kubernetes is enabled
        if has_k8s {
          refresh_pods(cx);
          refresh_namespaces(cx);
          refresh_services(cx);
          refresh_deployments(cx);
        }
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to restart '{name_for_context}': {e}"),
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
  restart_machine(name, cx);
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

/// Refresh the list of Colima machines
pub fn refresh_machines(cx: &mut App) {
  let state = docker_state(cx);

  let task = cx
    .background_executor()
    .spawn(async move { ColimaClient::list().unwrap_or_default() });

  cx.spawn(async move |cx| {
    let vms = task.await;
    cx.update(|cx| {
      state.update(cx, |state, cx| {
        state.set_machines(vms);
        cx.emit(StateChanged::MachinesUpdated);
      });
    })
  })
  .detach();
}

/// Set a machine as the default by switching docker and k8s contexts
pub fn set_default_machine(name: String, has_kubernetes: bool, cx: &mut App) {
  let task_id = start_task(cx, format!("Setting '{name}' as default..."));

  let disp = dispatcher(cx);
  let state = docker_state(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        // Docker context name for colima is "colima" for default or "colima-<profile>" for others
        let context_name = if name == "default" {
          "colima".to_string()
        } else {
          format!("colima-{name}")
        };

        // Switch docker context
        let docker_output = docker_cmd().args(["context", "use", &context_name]).output();

        match &docker_output {
          Err(e) => return Err(format!("Failed to switch docker context: {e}")),
          Ok(out) if !out.status.success() => {
            return Err(format!(
              "Failed to switch docker context: {}",
              String::from_utf8_lossy(&out.stderr)
            ));
          }
          Ok(_) => {}
        }

        // If machine has kubernetes, switch kubectl context
        if has_kubernetes {
          // kubectl context for colima is "colima" for default or "colima-<profile>" for others
          let kubectl_context = context_name.clone();

          // k8s context switch is optional - don't fail if kubectl isn't available
          let _ = kubectl_cmd().args(["config", "use-context", &kubectl_context]).output();
        }

        Ok((context_name, has_kubernetes))
      })
      .await;

    cx.update(|cx| match result {
      Ok((context_name, switched_k8s)) => {
        complete_task(cx, task_id);

        let msg = if switched_k8s {
          format!("'{context_name}' is now the default (Docker + Kubernetes)")
        } else {
          format!("'{context_name}' is now the default")
        };

        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });

        // Refresh data to reflect new context
        refresh_containers(cx);
        if switched_k8s {
          refresh_pods(cx);
          refresh_namespaces(cx);
          refresh_services(cx);
          refresh_deployments(cx);
        }

        // Notify that default machine changed
        state.update(cx, |_, cx| {
          cx.emit(StateChanged::MachinesUpdated);
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to set default: {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Update the container runtime in a machine
pub fn update_machine_runtime(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Updating runtime on '{name}'..."));
  let name_clone = name.clone();

  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };
        ColimaClient::update(name_opt)
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Runtime updated on '{name_clone}'"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to update runtime on '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Update runtime on all running machines
pub fn update_all_machines(cx: &mut App) {
  let task_id = start_task(cx, "Updating all machines...".to_string());

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  // Get list of running machines
  let machines = state.read(cx).colima_vms.clone();
  let running_machines: Vec<_> = machines
    .iter()
    .filter(|m| m.status.is_running())
    .map(|m| m.name.clone())
    .collect();

  if running_machines.is_empty() {
    complete_task(cx, task_id);
    disp.update(cx, |_, cx| {
      cx.emit(DispatcherEvent::TaskCompleted {
        message: "No running machines to update".to_string(),
      });
    });
    return;
  }

  let count = running_machines.len();

  cx.spawn(async move |cx| {
    let mut success_count = 0;
    let mut failed_names = Vec::new();

    for name in running_machines {
      let name_clone = name.clone();
      let result = cx
        .background_executor()
        .spawn(async move {
          let name_opt = if name == "default" { None } else { Some(name.as_str()) };
          ColimaClient::update(name_opt)
        })
        .await;

      if result.is_ok() {
        success_count += 1;
      } else {
        failed_names.push(name_clone);
      }
    }

    cx.update(|cx| {
      if failed_names.is_empty() {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Updated {success_count} machine(s)"),
          });
        });
      } else {
        fail_task(cx, task_id, format!("Failed: {}", failed_names.join(", ")));
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Updated {success_count}/{count}, failed: {}", failed_names.join(", ")),
          });
        });
      }
    })
  })
  .detach();
}

/// Prune Colima cached assets
pub fn prune_cache(all: bool, cx: &mut App) {
  let task_id = start_task(
    cx,
    if all {
      "Pruning all cached assets...".to_string()
    } else {
      "Pruning cached assets...".to_string()
    },
  );

  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move { ColimaClient::prune(all, true) })
      .await;

    cx.update(|cx| match result {
      Ok(output) => {
        complete_task(cx, task_id);
        let msg = if output.trim().is_empty() {
          "Cache pruned successfully".to_string()
        } else {
          format!("Cache pruned: {}", output.trim())
        };
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to prune cache: {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Run a provision script on a machine
pub fn run_provision_script(name: String, script: String, as_root: bool, cx: &mut App) {
  let task_id = start_task(cx, format!("Running script on '{name}'..."));
  let name_clone = name.clone();

  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let name_opt = if name == "default" { None } else { Some(name.as_str()) };
        ColimaClient::run_provision_script(name_opt, &script, as_root)
      })
      .await;

    cx.update(|cx| match result {
      Ok(output) => {
        complete_task(cx, task_id);
        let msg = if output.trim().is_empty() {
          format!("Script executed on '{name_clone}'")
        } else {
          format!("Script output:\n{}", output.trim())
        };
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Script failed on '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Open a machine folder in an external editor (VS Code, Cursor, or Zed)
///
/// For VS Code and Cursor, uses the SSH Remote extension.
/// For Zed, uses SSH remote connection.
///
/// # Arguments
/// * `profile` - The colima profile name
/// * `path` - The path inside the machine to open
pub fn open_machine_in_editor(profile: &str, path: &str, cx: &mut App) {
  use crate::state::settings_state;
  use std::process::Command;

  let settings = settings_state(cx).read(cx);
  let editor = settings.settings.external_editor.clone();
  let disp = dispatcher(cx);

  // Build SSH host from colima profile
  // Colima sets up SSH config entries like "colima" or "colima-<profile>"
  let ssh_host = if profile == "default" {
    "colima".to_string()
  } else {
    format!("colima-{profile}")
  };

  // Handle VS Code / Cursor with SSH Remote
  if editor.supports_container_attach() {
    // VS Code/Cursor use: code --remote ssh-remote+<host> <path>
    let command = editor.command();

    let result = Command::new(command)
      .arg("--remote")
      .arg(format!("ssh-remote+{ssh_host}"))
      .arg(path)
      .spawn();

    match result {
      Ok(_) => {
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Opening machine in {} via SSH", editor.display_name()),
          });
        });
      }
      Err(e) => {
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to open {}: {e}", editor.display_name()),
          });
        });
      }
    }
    return;
  }

  // Handle Zed with SSH
  if editor.supports_ssh() {
    // Build SSH URL for Zed: zed ssh://<host>/<path>
    let ssh_url = format!("ssh://{ssh_host}{path}");

    let result = Command::new(editor.command()).arg(ssh_url).spawn();

    match result {
      Ok(_) => {
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Opening machine in {} via SSH", editor.display_name()),
          });
        });
      }
      Err(e) => {
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to open {}: {e}", editor.display_name()),
          });
        });
      }
    }
  }
}
