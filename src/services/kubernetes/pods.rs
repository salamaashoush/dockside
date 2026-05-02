//! Kubernetes pod operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

/// Refresh the list of pods
pub fn refresh_pods(cx: &mut App) {
  use crate::state::LoadState;

  let state = docker_state(cx);

  let namespace = state.read(cx).selected_namespace.clone();
  let ns_filter = if namespace == "all" { None } else { Some(namespace) };

  // Only show loading state on initial load, not on background refreshes
  let is_initial_load = matches!(state.read(cx).pods_state, LoadState::NotLoaded);
  if is_initial_load {
    state.update(cx, |state, _cx| {
      state.set_pods_loading();
    });
  }

  let tokio_task = Tokio::spawn(cx, async move {
    // Check both client creation AND actual API connectivity
    match crate::kubernetes::KubeClient::new().await {
      Ok(client) => {
        // Only set available=true if we can actually reach the K8s API
        match client.list_pods(ns_filter.as_deref()).await {
          Ok(pods) => Ok((true, pods)),
          Err(e) => Err(format!("Failed to list pods: {e}")),
        }
      }
      Err(e) => Err(format!("Failed to connect to Kubernetes: {e}")),
    }
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;

    cx.update(|cx| {
      state.update(cx, |state, cx| {
        match result {
          Ok(Ok((available, pods))) => {
            state.set_k8s_available(available);
            state.set_k8s_error(None);
            state.set_pods(pods);
          }
          Ok(Err(e)) => {
            state.set_k8s_available(false);
            state.set_k8s_error(Some(e.clone()));
            state.set_pods_error(e);
          }
          Err(join_err) => {
            let error_msg = join_err.to_string();
            state.set_k8s_available(false);
            state.set_k8s_error(Some(error_msg.clone()));
            state.set_pods_error(error_msg);
          }
        }
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
    // Check both client creation AND actual API connectivity
    match crate::kubernetes::KubeClient::new().await {
      Ok(client) => {
        // Only set available=true if we can actually reach the K8s API
        match client.list_namespaces().await {
          Ok(namespaces) => {
            let ns_names = namespaces.into_iter().map(|ns| ns.name).collect();
            (true, ns_names)
          }
          Err(_) => (false, vec!["default".to_string()]),
        }
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

/// Set the selected namespace for k8s filtering. Refreshes every k8s
/// resource list so each view sees the new scope.
pub fn set_namespace(namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |state, cx| {
    state.set_selected_namespace(namespace);
    cx.emit(StateChanged::NamespacesUpdated);
  });
  refresh_pods(cx);
  super::deployments::refresh_deployments(cx);
  super::services::refresh_services(cx);
  super::secrets::refresh_secrets(cx);
  super::configmaps::refresh_configmaps(cx);
  super::statefulsets::refresh_statefulsets(cx);
  super::daemonsets::refresh_daemonsets(cx);
  super::jobs::refresh_jobs(cx);
  super::cronjobs::refresh_cronjobs(cx);
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

/// Force delete a pod
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

/// Restart a pod
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
