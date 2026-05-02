//! Kubernetes `StatefulSet` operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{LoadState, StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

pub fn refresh_statefulsets(cx: &mut App) {
  let state = docker_state(cx);
  let is_initial = matches!(state.read(cx).statefulsets_state, LoadState::NotLoaded);
  if is_initial {
    state.update(cx, |s, _| s.statefulsets_state = LoadState::Loading);
  }
  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_statefulsets(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |s, cx| match result {
        Ok(Ok(items)) => {
          s.statefulsets = items;
          s.statefulsets_state = LoadState::Loaded;
          cx.emit(StateChanged::StatefulSetsUpdated);
        }
        Ok(Err(e)) => s.statefulsets_state = LoadState::Error(e.to_string()),
        Err(e) => s.statefulsets_state = LoadState::Error(e.to_string()),
      });
    })
  })
  .detach();
}

pub fn delete_statefulset(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting statefulset '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_statefulset(&name, &namespace).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("StatefulSet '{label}' deleted"),
          });
        });
        refresh_statefulsets(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete statefulset '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn scale_statefulset(name: String, namespace: String, replicas: i32, cx: &mut App) {
  let task_id = start_task(cx, format!("Scaling statefulset '{name}' to {replicas}..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.scale_statefulset(&name, &namespace, replicas).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("StatefulSet '{label}' scaled to {replicas}"),
          });
        });
        refresh_statefulsets(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to scale statefulset '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn rollout_restart_statefulset(name: String, namespace: String, cx: &mut App) {
  rollout_restart_kind("StatefulSet", name, namespace, cx);
}

pub fn rollout_restart_kind(kind: &'static str, name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Restarting {kind} '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.rollout_restart_apps(kind, &name, &namespace).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("{kind} '{label}' restarted"),
          });
        });
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to restart {kind} '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn get_statefulset_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.get_statefulset_yaml(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let yaml = match result {
      Ok(y) => y,
      Err(e) => format!("Error: {e}"),
    };

    cx.update(|cx| {
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::StatefulSetYamlLoaded {
          name: name_clone,
          namespace: namespace_clone,
          yaml,
        });
      });
    })
  })
  .detach();
}

pub fn apply_statefulset_yaml(name: String, namespace: String, yaml: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Applying statefulset '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.apply_statefulset_yaml(&name, &namespace, &yaml).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("StatefulSet '{label}' applied"),
          });
        });
        refresh_statefulsets(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to apply statefulset '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}
