//! Kubernetes `PersistentVolumeClaim` operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{LoadState, StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

pub fn refresh_pvcs(cx: &mut App) {
  let state = docker_state(cx);
  let is_initial = matches!(state.read(cx).pvcs_state, LoadState::NotLoaded);
  if is_initial {
    state.update(cx, |s, _| s.pvcs_state = LoadState::Loading);
  }
  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_pvcs(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |s, cx| match result {
        Ok(Ok(items)) => {
          s.pvcs = items;
          s.pvcs_state = LoadState::Loaded;
          cx.emit(StateChanged::PvcsUpdated);
        }
        Ok(Err(e)) => s.pvcs_state = LoadState::Error(e.to_string()),
        Err(e) => s.pvcs_state = LoadState::Error(e.to_string()),
      });
    })
  })
  .detach();
}

pub fn delete_pvc(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting PVC '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_pvc(&name, &namespace).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("PVC '{label}' deleted"),
          });
        });
        refresh_pvcs(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete PVC '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}
