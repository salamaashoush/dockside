//! Kubernetes `ConfigMap` operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{LoadState, StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

pub fn refresh_configmaps(cx: &mut App) {
  let state = docker_state(cx);
  let is_initial = matches!(state.read(cx).configmaps_state, LoadState::NotLoaded);
  if is_initial {
    state.update(cx, |s, _| {
      s.configmaps_state = LoadState::Loading;
    });
  }

  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_configmaps(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |s, cx| match result {
        Ok(Ok(items)) => {
          s.configmaps = items;
          s.configmaps_state = LoadState::Loaded;
          cx.emit(StateChanged::ConfigMapsUpdated);
        }
        Ok(Err(e)) => {
          s.configmaps_state = LoadState::Error(e.to_string());
        }
        Err(e) => {
          s.configmaps_state = LoadState::Error(e.to_string());
        }
      });
    })
  })
  .detach();
}

pub fn delete_configmap(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting configmap '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_configmap(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("ConfigMap '{label}' deleted"),
          });
        });
        refresh_configmaps(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete configmap '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn load_configmap_entries(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let label = name.clone();
  let ns = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.read_configmap_entries(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let entries = match result {
      Ok(v) => v,
      Err(e) => vec![("error".to_string(), e.to_string())],
    };
    cx.update(|cx| {
      state.update(cx, |_s, cx| {
        cx.emit(StateChanged::ConfigMapEntriesLoaded {
          name: label,
          namespace: ns,
          entries,
        });
      });
    })
  })
  .detach();
}
