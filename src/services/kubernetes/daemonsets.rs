//! Kubernetes `DaemonSet` operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{LoadState, StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

pub fn refresh_daemonsets(cx: &mut App) {
  let state = docker_state(cx);
  let is_initial = matches!(state.read(cx).daemonsets_state, LoadState::NotLoaded);
  if is_initial {
    state.update(cx, |s, _| s.daemonsets_state = LoadState::Loading);
  }
  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_daemonsets(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |s, cx| match result {
        Ok(Ok(items)) => {
          s.daemonsets = items;
          s.daemonsets_state = LoadState::Loaded;
          cx.emit(StateChanged::DaemonSetsUpdated);
        }
        Ok(Err(e)) => s.daemonsets_state = LoadState::Error(e.to_string()),
        Err(e) => s.daemonsets_state = LoadState::Error(e.to_string()),
      });
    })
  })
  .detach();
}

pub fn delete_daemonset(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting daemonset '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_daemonset(&name, &namespace).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("DaemonSet '{label}' deleted"),
          });
        });
        refresh_daemonsets(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete daemonset '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn rollout_restart_daemonset(name: String, namespace: String, cx: &mut App) {
  super::statefulsets::rollout_restart_kind("DaemonSet", name, namespace, cx);
}
