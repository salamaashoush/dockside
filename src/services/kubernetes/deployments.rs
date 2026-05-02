//! Kubernetes deployment operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};
use super::pods::refresh_pods;

/// Refresh deployments list
pub fn refresh_deployments(cx: &mut App) {
  use crate::state::LoadState;

  let state = docker_state(cx);

  // Only show loading state on initial load, not on background refreshes
  let is_initial_load = matches!(state.read(cx).deployments_state, LoadState::NotLoaded);
  if is_initial_load {
    state.update(cx, |state, _cx| {
      state.set_deployments_loading();
    });
  }

  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_deployments(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;

    cx.update(|cx| {
      state.update(cx, |state, cx| match result {
        Ok(Ok(deployments)) => {
          state.set_k8s_error(None);
          state.set_deployments(deployments);
          cx.emit(StateChanged::DeploymentsUpdated);
        }
        Ok(Err(e)) => {
          let error_msg = e.to_string();
          state.set_k8s_error(Some(error_msg.clone()));
          state.set_deployments_error(error_msg);
        }
        Err(join_err) => {
          let error_msg = join_err.to_string();
          state.set_k8s_error(Some(error_msg.clone()));
          state.set_deployments_error(error_msg);
        }
      });
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

/// Create a new Kubernetes deployment
pub fn apply_deployment_yaml(name: String, namespace: String, yaml: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Applying deployment '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.apply_deployment_yaml(&name, &namespace, &yaml).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Deployment '{label}' applied"),
          });
        });
        refresh_deployments(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to apply deployment '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn rollback_deployment(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Rolling back '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.rollback_deployment(&name, &namespace).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(rev) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Deployment '{label}' rolled back to revision {rev}"),
          });
        });
        refresh_deployments(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to roll back '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

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
