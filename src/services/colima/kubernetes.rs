//! Kubernetes control operations (start/stop/reset on Colima)

use gpui::App;

use crate::colima::ColimaClient;
use crate::services::{TaskStage, advance_stage, complete_task, fail_task, start_staged_task, start_task};
use crate::state::{StateChanged, docker_state};
use crate::utils::{colima_cmd, kubectl_cmd};

use super::super::core::{DispatcherEvent, dispatcher};
use super::super::kubernetes::{refresh_deployments, refresh_namespaces, refresh_pods, refresh_services};

/// Switch kubectl context (async, non-blocking)
pub fn switch_kubectl_context(context: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Switching to '{context}'..."));
  let disp = dispatcher(cx);
  let ctx = context.clone();

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let output = kubectl_cmd().args(["config", "use-context", &context]).output();
        match output {
          Ok(o) if o.status.success() => Ok(()),
          Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Switched to context '{ctx}'"),
          });
        });
        // Refresh K8s data with new context
        refresh_pods(cx);
        refresh_namespaces(cx);
        refresh_services(cx);
        refresh_deployments(cx);
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

/// Reset Kubernetes on Colima (async, non-blocking)
pub fn reset_colima_kubernetes(cx: &mut App) {
  let task_id = start_task(cx, "Resetting Kubernetes...".to_string());
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    let result = cx
      .background_executor()
      .spawn(async move {
        let output = colima_cmd().args(["kubernetes", "reset"]).output();
        match output {
          Ok(o) if o.status.success() => Ok(()),
          Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
          Err(e) => Err(e.to_string()),
        }
      })
      .await;

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: "Kubernetes reset completed".to_string(),
          });
        });
        // Refresh K8s data
        refresh_pods(cx);
        refresh_namespaces(cx);
        refresh_services(cx);
        refresh_deployments(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.clone());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to reset Kubernetes: {e}"),
          });
        });
      }
    })
  })
  .detach();
}

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
        let mut cmd = colima_cmd();
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
        let mut cmd = colima_cmd();
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
        let mut cmd = colima_cmd();
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

/// Enable Kubernetes on a Colima machine by restarting it with --kubernetes flag
/// This is for machines that were created without kubernetes support
pub fn enable_kubernetes(name: String, cx: &mut App) {
  // Create staged task with clear progress stages
  let stages = vec![
    TaskStage::new("stop", format!("Stopping '{name}'...")),
    TaskStage::new("start", format!("Starting '{name}' with Kubernetes...")),
    TaskStage::new("verify", format!("Verifying Kubernetes on '{name}'...")),
  ];

  let task_id = start_staged_task(cx, format!("Enabling K8s on '{name}'"), stages);
  let name_clone = name.clone();

  let state = docker_state(cx);
  let disp = dispatcher(cx);

  cx.spawn(async move |cx| {
    // Stage 0: Stop the machine
    let stop_result = cx
      .background_executor()
      .spawn({
        let name = name.clone();
        async move {
          let name_opt = if name == "default" { None } else { Some(name.as_str()) };
          ColimaClient::stop(name_opt)
        }
      })
      .await;

    if let Err(e) = stop_result {
      cx.update(|cx| {
        fail_task(cx, task_id, format!("Failed to stop: {e}"));
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to stop '{name_clone}': {e}"),
          });
        });
      })
      .ok();
      return;
    }

    // Stage 1: Start with kubernetes
    cx.update(|cx| advance_stage(cx, task_id)).ok();

    // Brief pause to let Colima release resources
    cx.background_executor()
      .timer(std::time::Duration::from_millis(500))
      .await;

    let start_result = cx
      .background_executor()
      .spawn({
        let name = name.clone();
        async move {
          // Read existing config and enable kubernetes
          let name_opt = if name == "default" { None } else { Some(name.as_str()) };
          let mut config = ColimaClient::read_config(name_opt).unwrap_or_default();
          config.kubernetes.enabled = true;
          ColimaClient::start_with_config(&name, &config)
        }
      })
      .await;

    if let Err(e) = start_result {
      cx.update(|cx| {
        fail_task(cx, task_id, format!("Failed to start with K8s: {e}"));
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to enable K8s on '{name_clone}': {e}"),
          });
        });
      })
      .ok();
      return;
    }

    // Stage 2: Verify and refresh
    cx.update(|cx| advance_stage(cx, task_id)).ok();

    // Refresh machine list
    let name_for_context = name.clone();
    let vms = cx
      .background_executor()
      .spawn(async move {
        let vms = ColimaClient::list().unwrap_or_default();

        // Switch kubectl context to the colima context for this machine
        let kubectl_context = if name_for_context == "default" {
          "colima".to_string()
        } else {
          format!("colima-{name_for_context}")
        };

        // Try to switch kubectl context (don't fail if it doesn't work)
        let _ = kubectl_cmd().args(["config", "use-context", &kubectl_context]).output();

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
          message: format!("Kubernetes enabled on '{name_clone}'"),
        });
      });
      // Refresh K8s data
      refresh_pods(cx);
      refresh_namespaces(cx);
      refresh_services(cx);
      refresh_deployments(cx);
    })
    .ok();
  })
  .detach();
}
